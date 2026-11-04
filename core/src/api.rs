use crate::acceptance_context::AcceptanceContext;
use crate::cheque_acceptance::ChequeAcceptance;
use crate::dtn_buffer::DtnEngine;
use crate::dtn_ingest_outcome::IngestOutcome;
use crate::dtn_peer_session::DtnPeerSession;
use crate::dtn_snapshot::verify_snapshot;
use crate::issuer_trust::IssuerTrust;
use crate::lab_issuer;
use crate::payer_tx_engine::PayerEngine;
use crate::prepared_ack::PreparedAck;
use crate::prepared_propose::PreparedPropose;
use crate::protocol_clock::{MAX_FORWARD_JUMP_SECONDS, ProtocolClock, VerifiedServerTime};
use crate::protocol_now::ProtocolNow;
use crate::receiver_clock_row::ReceiverClockRow;
use crate::receiver_tx_engine::ReceiverEngine;
use crate::tx_keys::{
  compressed_public_key, compressed_public_key_of, signing_key_from_seed, verifying_key_from_sec1,
};
use crate::tx_wire::cheque_stub_body::ChequeStubBody;
use crate::tx_wire::sync_envelope::SyncEnvelope;
use flutter_rust_bridge::frb;
use p256::ecdsa::{
  Signature, SigningKey, VerifyingKey,
  signature::{Signer, Verifier},
};
use prost::Message;
use rusqlite::Connection;
use snow::params::{DHChoice, NoiseParams};
use snow::resolvers::{CryptoResolver, DefaultResolver};
use snow::{Builder, HandshakeState, TransportState};
use std::fs::{self, OpenOptions};
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;
use zeroize::Zeroizing;

#[frb(sync)]
pub fn core_version() -> String {
  env!("CARGO_PKG_VERSION").to_owned()
}

#[frb(sync)]
pub fn ping(n: u32) -> u32 {
  n + 1
}

pub fn echo(data: Vec<u8>) -> Vec<u8> {
  data
}

const NOISE_XK_PROTOCOL: &str = "Noise_XK_25519_ChaChaPoly_BLAKE2s";
const NOISE_MAX_MESSAGE_LEN: usize = 65535;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoiseKeypair {
  pub public_key: Vec<u8>,
  pub private_key: Vec<u8>,
}

/// Fresh X25519 static keypair; the receiver calls this once per charge.
#[frb(sync)]
pub fn generate_noise_static_keypair() -> Result<NoiseKeypair, String> {
  let keypair = Builder::new(noise_xk_params()?)
    .generate_keypair()
    .map_err(noise_error)?;

  Ok(NoiseKeypair {
    public_key: keypair.public,
    private_key: keypair.private,
  })
}

enum NoiseStage {
  Handshake(Box<HandshakeState>),
  Transport(Box<TransportState>),
  Failed,
}

pub struct NoiseSession {
  stage: NoiseStage,
  is_initiator: bool,
  local_public_key: Vec<u8>,
}

impl NoiseSession {
  /// Prologue = all QR data, byte for byte as scanned.
  #[frb(sync)]
  pub fn new_initiator(
    responder_public_key: Vec<u8>,
    prologue: Vec<u8>,
  ) -> Result<NoiseSession, String> {
    // The initiator's own static is required by the XK pattern, not by the protocol of this app:
    // it is generated here and discarded with the session.
    let params = noise_xk_params()?;
    let local = Builder::new(params.clone())
      .generate_keypair()
      .map_err(noise_error)?;
    let state = Builder::new(params)
      .local_private_key(&local.private)
      .and_then(|builder| builder.remote_public_key(&responder_public_key))
      .and_then(|builder| builder.prologue(&prologue))
      .and_then(|builder| builder.build_initiator())
      .map_err(noise_error)?;

    Ok(NoiseSession {
      stage: NoiseStage::Handshake(Box::new(state)),
      is_initiator: true,
      local_public_key: local.public,
    })
  }

  #[frb(sync)]
  pub fn new_responder(
    local_private_key: Vec<u8>,
    prologue: Vec<u8>,
  ) -> Result<NoiseSession, String> {
    let state = Builder::new(noise_xk_params()?)
      .local_private_key(&local_private_key)
      .and_then(|builder| builder.prologue(&prologue))
      .and_then(|builder| builder.build_responder())
      .map_err(noise_error)?;
    let mut dh = DefaultResolver
      .resolve_dh(&DHChoice::Curve25519)
      .ok_or("noise: no X25519")?;
    dh.set(&local_private_key);

    Ok(NoiseSession {
      stage: NoiseStage::Handshake(Box::new(state)),
      is_initiator: false,
      local_public_key: dh.pubkey().to_vec(),
    })
  }

  #[frb(sync)]
  pub fn is_finished(&self) -> bool {
    matches!(self.stage, NoiseStage::Transport(_))
  }

  /// Initiator static key || responder static key of the finished handshake: the same bytes on
  /// both ends, authenticated by Noise. Key for `DtnBuffer::open_session`.
  #[frb(sync)]
  pub fn peer_pair_key(&self) -> Result<Vec<u8>, String> {
    let NoiseStage::Transport(state) = &self.stage else {
      return Err("noise handshake is not finished".to_owned());
    };
    let remote = state.get_remote_static().ok_or("noise: no remote static")?;
    let local = self.local_public_key.as_slice();
    Ok(if self.is_initiator {
      [local, remote].concat()
    } else {
      [remote, local].concat()
    })
  }

  pub fn write_message(&mut self, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    self.step_handshake(|state, buffer| state.write_message(&payload, buffer))
  }

  pub fn read_message(&mut self, message: Vec<u8>) -> Result<Vec<u8>, String> {
    self.step_handshake(|state, buffer| state.read_message(&message, buffer))
  }

  pub fn encrypt(&mut self, plaintext: Vec<u8>) -> Result<Vec<u8>, String> {
    self.transport_step(|state, buffer| state.write_message(&plaintext, buffer))
  }

  pub fn decrypt(&mut self, ciphertext: Vec<u8>) -> Result<Vec<u8>, String> {
    self.transport_step(|state, buffer| state.read_message(&ciphertext, buffer))
  }

  fn step_handshake(
    &mut self,
    operation: impl FnOnce(&mut HandshakeState, &mut [u8]) -> Result<usize, snow::Error>,
  ) -> Result<Vec<u8>, String> {
    let NoiseStage::Handshake(mut state) = std::mem::replace(&mut self.stage, NoiseStage::Failed)
    else {
      return Err("noise handshake is not in progress".to_owned());
    };
    let mut buffer = vec![0; NOISE_MAX_MESSAGE_LEN];
    // A failed step leaves the stage as Failed: a bad message ends the session.
    let length = operation(&mut state, &mut buffer).map_err(noise_error)?;
    buffer.truncate(length);

    self.stage = if state.is_handshake_finished() {
      NoiseStage::Transport(Box::new(state.into_transport_mode().map_err(noise_error)?))
    } else {
      NoiseStage::Handshake(state)
    };

    Ok(buffer)
  }

  fn transport_step(
    &mut self,
    operation: impl FnOnce(&mut TransportState, &mut [u8]) -> Result<usize, snow::Error>,
  ) -> Result<Vec<u8>, String> {
    let NoiseStage::Transport(state) = &mut self.stage else {
      return Err("noise handshake is not finished".to_owned());
    };
    let mut buffer = vec![0; NOISE_MAX_MESSAGE_LEN];
    let length = operation(state, &mut buffer).map_err(noise_error)?;
    buffer.truncate(length);

    Ok(buffer)
  }
}

fn noise_xk_params() -> Result<NoiseParams, String> {
  NOISE_XK_PROTOCOL.parse().map_err(noise_error)
}

fn noise_error(error: snow::Error) -> String {
  format!("noise: {error}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreSelfcheck {
  pub sqlcipher_opens_with_key: bool,
  pub sqlcipher_rejects_without_key: bool,
  pub noise_xk_handshake_ok: bool,
  pub p256_sign_verify_ok: bool,
  pub rust_version: String,
  pub elapsed_micros: u64,
}

pub fn core_selfcheck(storage_dir: String) -> CoreSelfcheck {
  let started = Instant::now();
  let rust_version = rustc_version_runtime::version().to_string();
  let database_path = create_selfcheck_database_path(Path::new(&storage_dir));
  let checks = catch_unwind(AssertUnwindSafe(|| {
    let (sqlcipher_opens_with_key, sqlcipher_rejects_without_key) = database_path
      .as_deref()
      .map(run_sqlcipher_selfcheck)
      .unwrap_or((false, false));

    (
      sqlcipher_opens_with_key,
      sqlcipher_rejects_without_key,
      run_noise_xk_selfcheck(),
      run_p256_selfcheck(),
    )
  }))
  .unwrap_or((false, false, false, false));
  let cleanup_succeeded = database_path
    .as_deref()
    .map(remove_selfcheck_database)
    .unwrap_or(false);

  CoreSelfcheck {
    sqlcipher_opens_with_key: checks.0 && cleanup_succeeded,
    sqlcipher_rejects_without_key: checks.1 && cleanup_succeeded,
    noise_xk_handshake_ok: checks.2,
    p256_sign_verify_ok: checks.3,
    rust_version,
    elapsed_micros: started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
  }
}

fn create_selfcheck_database_path(storage_dir: &Path) -> Option<PathBuf> {
  fs::create_dir_all(storage_dir).ok()?;

  let timestamp = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .ok()?
    .as_nanos();
  for sequence in 0..u32::MAX {
    let path = storage_dir.join(format!(
      "meshpay-selfcheck-{}-{timestamp}-{sequence}.sqlite",
      std::process::id()
    ));
    match OpenOptions::new().write(true).create_new(true).open(&path) {
      Ok(_) => return Some(path),
      Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
      Err(_) => return None,
    }
  }

  None
}

fn run_sqlcipher_selfcheck(database_path: &Path) -> (bool, bool) {
  const DATABASE_KEY: &str = "meshpay-core-selfcheck-key";

  let initial_open_succeeded = (|| {
    let connection = Connection::open(database_path).ok()?;
    connection.pragma_update(None, "key", DATABASE_KEY).ok()?;
    connection
      .execute_batch("CREATE TABLE selfcheck (value TEXT NOT NULL); INSERT INTO selfcheck (value) VALUES ('ok');")
      .ok()?;
    read_selfcheck_value(&connection).then_some(())
  })()
  .is_some();
  if !initial_open_succeeded {
    return (false, false);
  }

  let no_key_was_rejected = match Connection::open(database_path) {
    Ok(connection) => !read_selfcheck_value(&connection),
    Err(_) => true,
  };
  let keyed_reopen_succeeded = (|| {
    let connection = Connection::open(database_path).ok()?;
    connection.pragma_update(None, "key", DATABASE_KEY).ok()?;
    read_selfcheck_value(&connection).then_some(())
  })()
  .is_some();

  (keyed_reopen_succeeded, no_key_was_rejected)
}

fn read_selfcheck_value(connection: &Connection) -> bool {
  connection
    .query_row("SELECT value FROM selfcheck", [], |row| {
      row.get::<_, String>(0)
    })
    .is_ok_and(|value| value == "ok")
}

fn remove_selfcheck_database(database_path: &Path) -> bool {
  [
    database_path.as_os_str().to_os_string(),
    append_path_suffix(database_path, "-journal"),
    append_path_suffix(database_path, "-wal"),
    append_path_suffix(database_path, "-shm"),
  ]
  .into_iter()
  .all(|path| match fs::remove_file(path) {
    Ok(()) => true,
    Err(error) => error.kind() == io::ErrorKind::NotFound,
  })
}

fn append_path_suffix(path: &Path, suffix: &str) -> std::ffi::OsString {
  let mut path = path.as_os_str().to_os_string();
  path.push(suffix);
  path
}

fn run_noise_xk_selfcheck() -> bool {
  (|| {
    let params = "Noise_XK_25519_ChaChaPoly_BLAKE2s"
      .parse::<snow::params::NoiseParams>()
      .ok()?;
    let responder_builder = Builder::new(params.clone());
    let responder_keys = responder_builder.generate_keypair().ok()?;
    let initiator_builder = Builder::new(params.clone());
    let initiator_keys = initiator_builder.generate_keypair().ok()?;
    let mut initiator = initiator_builder
      .local_private_key(&initiator_keys.private)
      .ok()?
      .remote_public_key(&responder_keys.public)
      .ok()?
      .build_initiator()
      .ok()?;
    let mut responder = responder_builder
      .local_private_key(&responder_keys.private)
      .ok()?
      .build_responder()
      .ok()?;
    let mut message = [0; 1024];
    let mut payload = [0; 1024];

    let length = initiator.write_message(&[], &mut message).ok()?;
    responder
      .read_message(&message[..length], &mut payload)
      .ok()?;
    let length = responder.write_message(&[], &mut message).ok()?;
    initiator
      .read_message(&message[..length], &mut payload)
      .ok()?;
    let length = initiator.write_message(&[], &mut message).ok()?;
    responder
      .read_message(&message[..length], &mut payload)
      .ok()?;

    let mut initiator_transport = initiator.into_transport_mode().ok()?;
    let mut responder_transport = responder.into_transport_mode().ok()?;

    let initiator_message = b"initiator-to-responder";
    let length = initiator_transport
      .write_message(initiator_message, &mut message)
      .ok()?;
    let received_length = responder_transport
      .read_message(&message[..length], &mut payload)
      .ok()?;
    if &payload[..received_length] != initiator_message {
      return None;
    }

    let responder_message = b"responder-to-initiator";
    let length = responder_transport
      .write_message(responder_message, &mut message)
      .ok()?;
    let received_length = initiator_transport
      .read_message(&message[..length], &mut payload)
      .ok()?;
    (&payload[..received_length] == responder_message).then_some(())
  })()
  .is_some()
}

fn run_p256_selfcheck() -> bool {
  const TEST_PRIVATE_KEY: [u8; 32] = [1; 32];
  const MESSAGE: &[u8] = b"meshpay core selfcheck";

  let Ok(signing_key) = SigningKey::from_bytes(&TEST_PRIVATE_KEY.into()) else {
    return false;
  };
  let signature: Signature = signing_key.sign(MESSAGE);
  signing_key
    .verifying_key()
    .verify(MESSAGE, &signature)
    .is_ok()
}

/// LAB ONLY (TEST unit, no real value): software P-256 key for the two-phase commit engines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabSigningKey {
  pub seed: Vec<u8>,
  pub public_key: Vec<u8>,
}

pub fn generate_lab_signing_key() -> Result<LabSigningKey, String> {
  let mut seed = Zeroizing::new([0u8; 32]);
  // A seed outside [1, n-1] is astronomically unlikely, but retry instead of failing.
  loop {
    getrandom::fill(&mut *seed).map_err(|error| format!("random: {error}"))?;
    if let Ok(signing_key) = signing_key_from_seed(&*seed) {
      return Ok(LabSigningKey {
        seed: seed.to_vec(),
        public_key: compressed_public_key(&signing_key),
      });
    }
  }
}

/// LAB ONLY: root key the lab receivers pin for ISSUER certs (seed public on purpose).
pub fn lab_issuer_root_public_key() -> Vec<u8> {
  lab_issuer::root_public_key()
}

/// LAB ONLY: lab ISSUER `ServerKeyCert` valid 30 days from `not_before_unix`.
pub fn lab_issuer_cert(not_before_unix: u64) -> Vec<u8> {
  lab_issuer::cert(not_before_unix)
}

/// LAB ONLY: `ChequeStub` sealed by the lab ISSUER key (the lab has no issuing server).
/// `signer_pubkey` (SEC1, 33 or 65 bytes) is stored compressed, as the receiver compares it.
pub fn lab_cheque_stub(
  cheque_id: Vec<u8>,
  signer_pubkey: Vec<u8>,
  max_amount_units: u64,
  expires_at_unix: u64,
) -> Result<Vec<u8>, String> {
  Ok(lab_issuer::seal_stub(ChequeStubBody {
    cheque_id,
    signer_pubkey: compressed_public_key_of(&verifying_key_from_sec1(&signer_pubkey)?),
    max_amount_units,
    expires_at_unix,
    ..Default::default()
  }))
}

/// Payer side of the two-phase commit (PRD 6.5). All messages are protobuf bytes.
pub struct PayerTxEngine {
  engine: PayerEngine,
}

impl PayerTxEngine {
  /// `db_key` is the raw 32-byte SQLCipher key; `payee_public_key` is a compressed P-256 key.
  pub fn open(
    db_path: String,
    db_key: Vec<u8>,
    signing_seed: Vec<u8>,
    payee_public_key: Vec<u8>,
  ) -> Result<PayerTxEngine, String> {
    Ok(PayerTxEngine {
      engine: PayerEngine::open(
        Path::new(&db_path),
        &db_key,
        &signing_seed,
        &payee_public_key,
      )?,
    })
  }

  /// Returns the `TxPropose` bytes to send, only after intent and LOCKED_OUTGOING are durable.
  pub fn propose(
    &self,
    spend_body: Vec<u8>,
    counter: u64,
    recipient_id: Vec<u8>,
    stub: Vec<u8>,
  ) -> Result<Vec<u8>, String> {
    self
      .engine
      .propose(&spend_body, counter, &recipient_id, &stub)
  }

  /// External-signer mode (Android Keystore): no seed. `own_public_key` is the Keystore P-256
  /// key (SEC1 uncompressed 65 bytes or compressed 33). Use `prepare_propose` + `finish_propose`.
  pub fn open_external(
    db_path: String,
    db_key: Vec<u8>,
    own_public_key: Vec<u8>,
    payee_public_key: Vec<u8>,
  ) -> Result<PayerTxEngine, String> {
    Ok(PayerTxEngine {
      engine: PayerEngine::open_external(
        Path::new(&db_path),
        &db_key,
        &own_public_key,
        &payee_public_key,
      )?,
    })
  }

  /// External phase 1: nothing is stored. Sign `message` in the Keystore, then `finish_propose`.
  pub fn prepare_propose(
    &self,
    spend_body: Vec<u8>,
    counter: u64,
    recipient_id: Vec<u8>,
    stub: Vec<u8>,
  ) -> Result<PreparedPropose, String> {
    self
      .engine
      .prepare_propose(&spend_body, counter, &recipient_id, &stub)
  }

  /// External phase 2: verifies `signature` (raw 64 or DER), writes LOCKED_OUTGOING durably,
  /// returns the `TxPropose` bytes.
  pub fn finish_propose(
    &self,
    prepared: PreparedPropose,
    signature: Vec<u8>,
  ) -> Result<Vec<u8>, String> {
    self.engine.finish_propose(&prepared, &signature)
  }

  pub fn on_ack(&self, ack: Vec<u8>) -> Result<(), String> {
    self.engine.on_ack(&ack)
  }

  /// Expires still-locked cheques signed at least `expiry_secs` before `now_unix`; returns the count.
  pub fn expire_locked(&self, now_unix: u64, expiry_secs: u64) -> Result<u32, String> {
    let expired = self.engine.expire_locked(now_unix, expiry_secs)?;
    Ok(u32::try_from(expired).unwrap_or(u32::MAX))
  }

  /// Stored `TxPropose` bytes of every still-locked cheque, for byte-identical redelivery.
  pub fn resume_pending(&self) -> Result<Vec<Vec<u8>>, String> {
    self.engine.resume_pending()
  }
}

/// Receiver side of the two-phase commit (PRD 6.5). Every new cheque is judged by the protocol
/// clock anchored on the latest verified `RevocationSnapshot` (`t_srv` = its `issued_at_unix`);
/// with no snapshot installed every cheque is refused.
/// Largest snapshot `issued_at_unix` accepted, so `t_srv + MAX_FORWARD_JUMP_SECONDS + 1` and every
/// persisted value still fit SQLite's i64.
const MAX_STORED_UNIX: u64 = i64::MAX as u64 - MAX_FORWARD_JUMP_SECONDS - 1;

pub struct ReceiverTxEngine {
  engine: ReceiverEngine,
  snapshot_key: VerifyingKey,
  expiry_secs: u64,
  /// Serializes read-modify-write of the clock row persisted in the engine database.
  clock_lock: Mutex<()>,
}

impl ReceiverTxEngine {
  /// `snapshot_public_key` is the compressed P-256 SNAPSHOT role key; `expiry_secs` is `E`;
  /// `issuer_cert` is the ISSUER `ServerKeyCert` chained to the pinned `issuer_root_public_key`,
  /// whose key must seal every incoming stub.
  #[allow(clippy::too_many_arguments)]
  pub fn open(
    db_path: String,
    db_key: Vec<u8>,
    signing_seed: Vec<u8>,
    payer_public_key: Vec<u8>,
    own_account_id: Vec<u8>,
    snapshot_public_key: Vec<u8>,
    expiry_secs: u64,
    issuer_root_public_key: Vec<u8>,
    issuer_cert: Vec<u8>,
  ) -> Result<ReceiverTxEngine, String> {
    Self::with_engine(
      ReceiverEngine::open(
        Path::new(&db_path),
        &db_key,
        &signing_seed,
        &payer_public_key,
        &own_account_id,
        IssuerTrust::new(&issuer_root_public_key, &issuer_cert)?,
      )?,
      &snapshot_public_key,
      expiry_secs,
    )
  }

  /// External-signer mode (Android Keystore): no seed. Flow: `store_propose`, `prepare_ack`,
  /// sign `message` in the Keystore, `finish_ack`.
  #[allow(clippy::too_many_arguments)]
  pub fn open_external(
    db_path: String,
    db_key: Vec<u8>,
    own_public_key: Vec<u8>,
    payer_public_key: Vec<u8>,
    own_account_id: Vec<u8>,
    snapshot_public_key: Vec<u8>,
    expiry_secs: u64,
    issuer_root_public_key: Vec<u8>,
    issuer_cert: Vec<u8>,
  ) -> Result<ReceiverTxEngine, String> {
    Self::with_engine(
      ReceiverEngine::open_external(
        Path::new(&db_path),
        &db_key,
        &own_public_key,
        &payer_public_key,
        &own_account_id,
        IssuerTrust::new(&issuer_root_public_key, &issuer_cert)?,
      )?,
      &snapshot_public_key,
      expiry_secs,
    )
  }

  fn with_engine(
    engine: ReceiverEngine,
    snapshot_public_key: &[u8],
    expiry_secs: u64,
  ) -> Result<ReceiverTxEngine, String> {
    if !(ChequeAcceptance::MIN_EXPIRY_SECS..=ChequeAcceptance::MAX_EXPIRY_SECS)
      .contains(&expiry_secs)
    {
      return Err("expiry E must be 24 to 168 hours".to_owned());
    }
    Ok(ReceiverTxEngine {
      engine,
      snapshot_key: verifying_key_from_sec1(snapshot_public_key)?,
      expiry_secs,
      clock_lock: Mutex::new(()),
    })
  }

  /// Verifies `snapshot` with the SNAPSHOT key and anchors the protocol clock on its
  /// `issued_at_unix` (monotonic reading from the native plugin). The state is persisted: an older
  /// snapshot is refused, the same one is a no-op, a newer one always re-anchors and clears the
  /// sticky reboot flag. No forward-jump cap: the SNAPSHOT signature is the trust root, and a cap
  /// would turn an old-but-genuine first snapshot into a lockout after a long offline gap.
  /// Residual: a validly signed far-future snapshot (leaked SNAPSHOT key) becomes a permanent
  /// floor for "now"; that is a total compromise handled by the kill switch. Returns the held
  /// snapshot's `issued_at_unix`.
  pub fn install_snapshot(
    &self,
    snapshot: Vec<u8>,
    monotonic_nanos: u64,
    boot_count: Option<u64>,
  ) -> Result<u64, String> {
    let issued_at = verify_snapshot(&snapshot, &self.snapshot_key)?.issued_at_unix;
    if issued_at > MAX_STORED_UNIX {
      return Err("snapshot issued_at out of range".to_owned());
    }
    if i64::try_from(monotonic_nanos).is_err() {
      return Err("monotonic reading out of range".to_owned());
    }
    let _guard = self.clock_lock()?;
    let max_now_unix = match self.engine.load_clock()? {
      Some(held) if issued_at < held.t_srv_unix => {
        return Err("snapshot older than the held one".to_owned());
      }
      Some(held) if issued_at == held.t_srv_unix => return Ok(issued_at),
      Some(held) => match Self::advance(&held, monotonic_nanos, boot_count) {
        Some((row, _)) => issued_at.max(row.max_now_unix),
        None => issued_at.max(held.max_now_unix),
      },
      None => issued_at,
    };
    self.engine.save_clock(&ReceiverClockRow {
      t_srv_unix: issued_at,
      anchor_unix: issued_at,
      monotonic_nanos,
      boot_count,
      max_now_unix,
      rebooted: false,
    })?;
    Ok(issued_at)
  }

  /// Returns the `AckReceipt` bytes; the same propose twice returns the same ack.
  pub fn on_propose(
    &self,
    propose: Vec<u8>,
    monotonic_nanos: u64,
    boot_count: Option<u64>,
  ) -> Result<Vec<u8>, String> {
    let context = self.context(monotonic_nanos, boot_count)?;
    self.engine.on_propose(&propose, &context)
  }

  /// Verifies and durably stores the propose (state RECEIVED); returns its tx_id.
  pub fn store_propose(
    &self,
    propose: Vec<u8>,
    monotonic_nanos: u64,
    boot_count: Option<u64>,
  ) -> Result<Vec<u8>, String> {
    let context = self.context(monotonic_nanos, boot_count)?;
    self.engine.store_propose(&propose, &context)
  }

  pub fn prepare_ack(&self, tx_id: Vec<u8>) -> Result<PreparedAck, String> {
    self.engine.prepare_ack(&tx_id)
  }

  /// Verifies `signature` (raw 64 or DER), persists and returns the `AckReceipt` bytes.
  pub fn finish_ack(&self, tx_id: Vec<u8>, signature: Vec<u8>) -> Result<Vec<u8>, String> {
    self.engine.finish_ack(&tx_id, &signature)
  }

  /// A suspected reboot (boot counter differs, `None` vs `Some` included, or monotonic below the
  /// anchor) only tightens: it is persisted (sticky until a newer snapshot), "now" continues from
  /// the largest "now" ever computed, and the `expires_at - t_srv <= E` bound applies. A reading
  /// putting "now" more than `MAX_FORWARD_JUMP_SECONDS` past `t_srv` (bogus monotonic value) is
  /// clamped just past that bound, so every cheque is refused as stale, and is never persisted.
  fn context(
    &self,
    monotonic_nanos: u64,
    boot_count: Option<u64>,
  ) -> Result<AcceptanceContext, String> {
    let _guard = self.clock_lock()?;
    let held = self.engine.load_clock()?.ok_or("no revocation snapshot")?;
    let now = match Self::advance(&held, monotonic_nanos, boot_count) {
      Some((row, now)) => {
        if row != held {
          self.engine.save_clock(&row)?;
        }
        now
      }
      None => ProtocolNow {
        unix: held.t_srv_unix + MAX_FORWARD_JUMP_SECONDS + 1,
        t_srv_unix: held.t_srv_unix,
        rebooted: true,
      },
    };
    Ok(AcceptanceContext {
      now,
      snapshot_issued_at_unix: Some(held.t_srv_unix),
      expiry_secs: self.expiry_secs,
    })
  }

  /// Next persisted state and "now" for this reading; `None` when the reading would push "now"
  /// beyond `t_srv + MAX_FORWARD_JUMP_SECONDS` (or the monotonic value cannot be stored).
  fn advance(
    held: &ReceiverClockRow,
    monotonic_nanos: u64,
    boot_count: Option<u64>,
  ) -> Option<(ReceiverClockRow, ProtocolNow)> {
    i64::try_from(monotonic_nanos).ok()?;
    let reading = ProtocolClock::new(
      VerifiedServerTime::from_verified_signature(held.anchor_unix),
      held.monotonic_nanos,
      held.boot_count,
    )
    .now(monotonic_nanos, boot_count);
    let row = if reading.rebooted {
      ReceiverClockRow {
        anchor_unix: held.max_now_unix,
        monotonic_nanos,
        boot_count,
        rebooted: true,
        ..*held
      }
    } else {
      ReceiverClockRow {
        max_now_unix: reading.unix.max(held.max_now_unix),
        ..*held
      }
    };
    if row.max_now_unix > held.t_srv_unix + MAX_FORWARD_JUMP_SECONDS {
      return None;
    }
    Some((
      row,
      ProtocolNow {
        unix: row.max_now_unix,
        t_srv_unix: held.t_srv_unix,
        rebooted: row.rebooted,
      },
    ))
  }

  fn clock_lock(&self) -> Result<MutexGuard<'_, ()>, String> {
    self
      .clock_lock
      .lock()
      .map_err(|_| "receiver clock lock poisoned".to_owned())
  }
}

#[cfg(test)]
mod tests {
  use super::{CoreSelfcheck, core_selfcheck, echo};
  use std::fs;

  #[test]
  fn echo_round_trips_25_kilobytes() {
    let input = (0..25 * 1024)
      .map(|index| (index % 256) as u8)
      .collect::<Vec<_>>();

    assert_eq!(echo(input.clone()), input);
  }

  #[test]
  fn core_selfcheck_checks_sqlcipher_noise_and_p256() {
    let storage_dir = tempfile::tempdir().expect("create storage directory");
    let result: CoreSelfcheck = core_selfcheck(storage_dir.path().to_string_lossy().into_owned());

    assert!(result.sqlcipher_opens_with_key);
    assert!(result.sqlcipher_rejects_without_key);
    assert!(result.noise_xk_handshake_ok);
    assert!(result.p256_sign_verify_ok);
    assert!(!result.rust_version.is_empty());
    assert!(
      fs::read_dir(storage_dir.path())
        .expect("read storage directory")
        .next()
        .is_none()
    );
  }
}

/// `SyncEnvelope` bytes for `encrypted_server_batch` (opaque to carriers); `envelope_id` is its hash.
#[frb(sync)]
pub fn build_dtn_envelope(
  encrypted_server_batch: Vec<u8>,
  created_at_unix: u64,
  ttl_hops: u32,
  known_revocation_version: u64,
) -> Vec<u8> {
  SyncEnvelope::new(
    encrypted_server_batch,
    created_at_unix,
    ttl_hops,
    known_revocation_version,
  )
  .encode_to_vec()
}

/// Third-party DTN envelope buffer (PRD 8.1), foreground gossip only. All messages are bytes.
pub struct DtnBuffer {
  engine: Arc<DtnEngine>,
}

impl DtnBuffer {
  /// `db_key` is the raw 32-byte SQLCipher key; `snapshot_public_key` is the compressed P-256
  /// SNAPSHOT role key; `max_ingest_per_minute` is the global ingest rate limit.
  pub fn open(
    db_path: String,
    db_key: Vec<u8>,
    snapshot_public_key: Vec<u8>,
    max_ingest_per_minute: u32,
  ) -> Result<DtnBuffer, String> {
    let db_key = Zeroizing::new(db_key);
    Ok(DtnBuffer {
      engine: Arc::new(DtnEngine::open(
        Path::new(&db_path),
        &db_key,
        &snapshot_public_key,
        max_ingest_per_minute,
      )?),
    })
  }

  /// One session per connected peer; all inbound envelopes go through it (20 per connection).
  /// `origin_pair` is `NoiseSession::peer_pair_key` of the authenticated link: envelopes count
  /// toward that pair's daily cap.
  #[frb(sync)]
  pub fn open_session(&self, origin_pair: Vec<u8>) -> DtnSession {
    DtnSession {
      session: DtnPeerSession::with_origin_pair(Arc::clone(&self.engine), origin_pair),
    }
  }

  /// Stores an envelope created on this device (not from a peer, so no pair cap).
  pub fn ingest_local(&self, envelope: Vec<u8>, now_unix: u64) -> Result<bool, String> {
    DtnSession::outcome(self.engine.ingest(&envelope, now_unix)?)
  }

  /// Held envelope ids (32 bytes each), capped at 256.
  pub fn summary(&self) -> Result<Vec<Vec<u8>>, String> {
    self.engine.summary()
  }

  /// Envelopes (bytes, one hop consumed) the peer lacks, at most 20.
  pub fn missing_from(&self, peer_summary: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>, String> {
    self.engine.missing_from(&peer_summary)
  }

  /// Envelopes for the first 20 requested ids.
  pub fn envelopes_for(&self, ids: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>, String> {
    self.engine.envelopes_for(&ids)
  }

  /// Deletes expired envelopes; returns how many.
  pub fn sweep(&self, now_unix: u64) -> Result<u32, String> {
    self.engine.sweep(now_unix)
  }

  /// Stores a `RevocationSnapshot` only if signed by the SNAPSHOT key and newer; returns its version.
  pub fn ingest_snapshot(&self, snapshot: Vec<u8>) -> Result<u64, String> {
    self.engine.ingest_snapshot(&snapshot)
  }

  pub fn snapshot_bytes(&self) -> Result<Option<Vec<u8>>, String> {
    self.engine.snapshot_bytes()
  }
}

/// Inbound side of one peer connection.
pub struct DtnSession {
  session: DtnPeerSession,
}

impl DtnSession {
  /// `Ok(true)` stored, `Ok(false)` duplicate; every other outcome (invalid, refused, rate
  /// limited, session budget used up) is `Err(reason)`.
  pub fn ingest(&self, envelope: Vec<u8>, now_unix: u64) -> Result<bool, String> {
    Self::outcome(self.session.ingest(&envelope, now_unix)?)
  }

  fn outcome(outcome: IngestOutcome) -> Result<bool, String> {
    match outcome {
      IngestOutcome::Stored => Ok(true),
      IngestOutcome::Duplicate => Ok(false),
      rejected => Err(format!("envelope rejected: {rejected:?}")),
    }
  }
}
