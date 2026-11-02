use flutter_rust_bridge::frb;
use p256::ecdsa::{
  Signature, SigningKey,
  signature::{Signer, Verifier},
};
use rusqlite::Connection;
use snow::{Builder, HandshakeState, TransportState, params::NoiseParams};
use std::fs::{self, OpenOptions};
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::Instant;

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
    let local_private_key = Builder::new(params.clone())
      .generate_keypair()
      .map_err(noise_error)?
      .private;
    let state = Builder::new(params)
      .local_private_key(&local_private_key)
      .and_then(|builder| builder.remote_public_key(&responder_public_key))
      .and_then(|builder| builder.prologue(&prologue))
      .and_then(|builder| builder.build_initiator())
      .map_err(noise_error)?;

    Ok(NoiseSession {
      stage: NoiseStage::Handshake(Box::new(state)),
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

    Ok(NoiseSession {
      stage: NoiseStage::Handshake(Box::new(state)),
    })
  }

  #[frb(sync)]
  pub fn is_finished(&self) -> bool {
    matches!(self.stage, NoiseStage::Transport(_))
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
