//! Shared receiver-acceptance fixtures: sealed server stub and ISSUER cert, protocol clock reading and SNAPSHOT key.
#![allow(dead_code)]

use meshpay_core::acceptance_context::AcceptanceContext;
use meshpay_core::dtn_snapshot::snapshot_signing_message;
use meshpay_core::issuer_trust::IssuerTrust;
use meshpay_core::protocol_now::ProtocolNow;
use meshpay_core::tx_keys::{compressed_public_key, sign, signing_key_from_seed};
use meshpay_core::tx_wire::cheque_spend_body::ChequeSpendBody;
use meshpay_core::tx_wire::cheque_stub::ChequeStub;
use meshpay_core::tx_wire::cheque_stub_body::ChequeStubBody;
use meshpay_core::tx_wire::revocation_snapshot::RevocationSnapshot;
use meshpay_core::tx_wire::server_key_cert::ServerKeyCert;
use meshpay_core::tx_wire::server_key_cert_body::ServerKeyCertBody;
use p256::ecdsa::SigningKey;
use prost::Message;
use std::sync::LazyLock;

pub const T_SRV: u64 = 1_900_000_000;
pub const E: u64 = 72 * 3600;
const SNAPSHOT_SEED: [u8; 32] = [5; 32];
const ROOT_SEED: [u8; 32] = [9; 32];
const ISSUER_SEED: [u8; 32] = [8; 32];
/// Every test payer signs with this seed; stubs are issued to its key.
const PAYER_SEED: [u8; 32] = [7; 32];
pub const CERT_NOT_BEFORE: u64 = T_SRV - 86_400;
pub const CERT_NOT_AFTER: u64 = CERT_NOT_BEFORE + 30 * 86_400;

pub fn issuer_root_public_key() -> Vec<u8> {
  compressed_public_key(&signing_key_from_seed(&ROOT_SEED).unwrap())
}

/// ISSUER cert signed by `root_seed` for `issuer_seed` with `role` and lifetime.
pub fn cert_with(
  root_seed: [u8; 32],
  issuer_seed: [u8; 32],
  role: i32,
  not_before: u64,
  not_after: u64,
) -> Vec<u8> {
  let pubkey = compressed_public_key(&signing_key_from_seed(&issuer_seed).unwrap());
  let body = ServerKeyCertBody {
    key_id: IssuerTrust::key_id(&pubkey),
    pubkey,
    role,
    not_before_unix: not_before,
    not_after_unix: not_after,
  }
  .encode_to_vec();
  let root = signing_key_from_seed(&root_seed).unwrap();
  ServerKeyCert {
    root_signature: sign(&root, &IssuerTrust::cert_message(&body)),
    body,
  }
  .encode_to_vec()
}

pub fn issuer_cert() -> Vec<u8> {
  cert_with(
    ROOT_SEED,
    ISSUER_SEED,
    IssuerTrust::ISSUER_ROLE,
    CERT_NOT_BEFORE,
    CERT_NOT_AFTER,
  )
}

pub fn issuer_trust() -> IssuerTrust {
  IssuerTrust::new(&issuer_root_public_key(), &issuer_cert()).unwrap()
}

/// Cached: debug-build scalar multiplications are slow enough to starve the SIGKILL loops.
static ISSUER: LazyLock<SigningKey> =
  LazyLock::new(|| signing_key_from_seed(&ISSUER_SEED).unwrap());
static ISSUER_KEY_ID: LazyLock<Vec<u8>> =
  LazyLock::new(|| IssuerTrust::key_id(&compressed_public_key(&ISSUER)));
static PAYER_PUBLIC_KEY: LazyLock<Vec<u8>> =
  LazyLock::new(|| compressed_public_key(&signing_key_from_seed(&PAYER_SEED).unwrap()));

/// Stub body for `cheque_id` issued to the test payer, any amount, sealed-key id set.
pub fn stub_body(cheque_id: &[u8], expires_at_unix: u64) -> ChequeStubBody {
  ChequeStubBody {
    cheque_id: cheque_id.to_vec(),
    signer_pubkey: PAYER_PUBLIC_KEY.clone(),
    max_amount_units: u64::MAX,
    expires_at_unix,
    server_key_id: ISSUER_KEY_ID.clone(),
    ..Default::default()
  }
}

/// `ChequeStub` bytes sealed by the test ISSUER key.
pub fn seal(body: &ChequeStubBody) -> Vec<u8> {
  let body = body.encode_to_vec();
  ChequeStub {
    server_signature: sign(&ISSUER, &IssuerTrust::stub_message(&body)),
    body,
  }
  .encode_to_vec()
}

/// Sealed `ChequeStub` bytes for `cheque_id` expiring at `expires_at_unix`.
pub fn stub_expiring(cheque_id: &[u8], expires_at_unix: u64) -> Vec<u8> {
  seal(&stub_body(cheque_id, expires_at_unix))
}

/// Valid stub (expires `E` after `T_SRV`) for the cheque named in `spend_body`.
pub fn stub_for(spend_body: &[u8]) -> Vec<u8> {
  let cheque_id = ChequeSpendBody::decode(spend_body)
    .map(|body| body.cheque_id)
    .unwrap_or_default();
  stub_expiring(&cheque_id, T_SRV + E)
}

/// Fresh snapshot at `T_SRV`, clock reading `T_SRV`, no reboot.
pub fn context() -> AcceptanceContext {
  AcceptanceContext {
    now: ProtocolNow {
      unix: T_SRV,
      t_srv_unix: T_SRV,
      rebooted: false,
    },
    snapshot_issued_at_unix: Some(T_SRV),
    expiry_secs: E,
  }
}

pub fn snapshot_cert() -> Vec<u8> {
  cert_with(
    ROOT_SEED,
    SNAPSHOT_SEED,
    IssuerTrust::SNAPSHOT_ROLE,
    CERT_NOT_BEFORE,
    CERT_NOT_AFTER,
  )
}

pub fn issuer_key_id() -> Vec<u8> {
  ISSUER_KEY_ID.clone()
}

pub fn signed_snapshot(issued_at_unix: u64) -> Vec<u8> {
  signed_snapshot_revoking(issued_at_unix, 1, Vec::new())
}

pub fn signed_snapshot_revoking(
  issued_at_unix: u64,
  version: u64,
  revoked_server_key_ids: Vec<Vec<u8>>,
) -> Vec<u8> {
  let mut snapshot = RevocationSnapshot {
    version,
    issued_at_unix,
    revoked_server_key_ids,
    ..Default::default()
  };
  let key = signing_key_from_seed(&SNAPSHOT_SEED).unwrap();
  snapshot.server_signature = sign(&key, &snapshot_signing_message(&snapshot));
  snapshot.encode_to_vec()
}

/// `stub_for` as a message, for hand-built `TxPropose`s.
pub fn stub_message_for(spend_body: &[u8]) -> ChequeStub {
  ChequeStub::decode(stub_for(spend_body).as_slice()).unwrap()
}
