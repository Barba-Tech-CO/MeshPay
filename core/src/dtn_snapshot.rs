use crate::tx_keys::verify;
use crate::tx_wire::revocation_snapshot::RevocationSnapshot;
use p256::ecdsa::VerifyingKey;
use prost::Message;

/// Message the SNAPSHOT key signs: domain prefix plus the snapshot with `server_signature` empty.
pub fn snapshot_signing_message(snapshot: &RevocationSnapshot) -> Vec<u8> {
  let unsigned = RevocationSnapshot {
    server_signature: Vec::new(),
    ..snapshot.clone()
  };
  [b"meshpay/snapshot/v1".as_slice(), &unsigned.encode_to_vec()].concat()
}

/// Parse limit (64 KiB) before decoding, then decodes and checks the signature. Version freshness is the caller's decision.
pub fn verify_snapshot(
  bytes: &[u8],
  snapshot_key: &VerifyingKey,
) -> Result<RevocationSnapshot, String> {
  if bytes.len() > 64 * 1024 {
    return Err("snapshot too large".to_owned());
  }
  let snapshot =
    RevocationSnapshot::decode(bytes).map_err(|error| format!("snapshot decode: {error}"))?;
  verify(
    snapshot_key,
    &snapshot_signing_message(&snapshot),
    &snapshot.server_signature,
  )
  .map_err(|_| "snapshot signature invalid".to_owned())?;
  Ok(snapshot)
}
