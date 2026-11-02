//! Mirrors core/src/tx_hashing.rs: the bytes the payer signs and the tx_id derivation.
use prost::encoding::decode_varint;
use sha2::{Digest, Sha256};

/// tx_id = SHA-256("meshpay/tx_id/v1" || counter (u64 big endian) || SHA-256(body without tx_id)).
pub fn derive_tx_id(counter: u64, body_bytes: &[u8]) -> Result<Vec<u8>, String> {
  let without_tx_id = strip_tx_id_field(body_bytes)?;
  let mut hasher = Sha256::new();
  hasher.update(b"meshpay/tx_id/v1");
  hasher.update(counter.to_be_bytes());
  hasher.update(Sha256::digest(without_tx_id));
  Ok(hasher.finalize().to_vec())
}

fn strip_tx_id_field(body_bytes: &[u8]) -> Result<Vec<u8>, String> {
  let malformed = || "malformed spend body".to_owned();
  let mut rest = body_bytes;
  let mut kept = Vec::with_capacity(body_bytes.len());
  while !rest.is_empty() {
    let field_start = rest;
    let key = decode_varint(&mut rest).map_err(|_| malformed())?;
    let skip = match key & 7 {
      0 => {
        decode_varint(&mut rest).map_err(|_| malformed())?;
        0
      }
      1 => 8,
      2 => usize::try_from(decode_varint(&mut rest).map_err(|_| malformed())?)
        .map_err(|_| malformed())?,
      5 => 4,
      _ => return Err(malformed()),
    };
    if skip > rest.len() {
      return Err(malformed());
    }
    rest = &rest[skip..];
    if key >> 3 != 4 {
      kept.extend_from_slice(&field_start[..field_start.len() - rest.len()]);
    }
  }
  Ok(kept)
}

/// Hash of the exact serialized `ChequeSpendBody` bytes.
pub fn spend_hash(body_bytes: &[u8]) -> Vec<u8> {
  Sha256::digest(body_bytes).to_vec()
}
