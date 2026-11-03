//! Shared receiver-acceptance fixtures: server stub, protocol clock reading and SNAPSHOT key.
#![allow(dead_code)]

use meshpay_core::acceptance_context::AcceptanceContext;
use meshpay_core::dtn_snapshot::snapshot_signing_message;
use meshpay_core::protocol_now::ProtocolNow;
use meshpay_core::tx_keys::{compressed_public_key, sign, signing_key_from_seed};
use meshpay_core::tx_wire::cheque_spend_body::ChequeSpendBody;
use meshpay_core::tx_wire::cheque_stub::ChequeStub;
use meshpay_core::tx_wire::cheque_stub_body::ChequeStubBody;
use meshpay_core::tx_wire::revocation_snapshot::RevocationSnapshot;
use prost::Message;

pub const T_SRV: u64 = 1_900_000_000;
pub const E: u64 = 72 * 3600;
const SNAPSHOT_SEED: [u8; 32] = [5; 32];

/// `ChequeStub` bytes for `cheque_id` expiring at `expires_at_unix` (signature not checked yet).
pub fn stub_expiring(cheque_id: &[u8], expires_at_unix: u64) -> Vec<u8> {
  ChequeStub {
    body: ChequeStubBody {
      cheque_id: cheque_id.to_vec(),
      expires_at_unix,
      ..Default::default()
    }
    .encode_to_vec(),
    server_signature: vec![0; 64],
  }
  .encode_to_vec()
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

pub fn snapshot_public_key() -> Vec<u8> {
  compressed_public_key(&signing_key_from_seed(&SNAPSHOT_SEED).unwrap())
}

pub fn signed_snapshot(issued_at_unix: u64) -> Vec<u8> {
  let mut snapshot = RevocationSnapshot {
    version: 1,
    issued_at_unix,
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
