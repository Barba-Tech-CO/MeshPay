use prost::encoding::{decode_varint, encode_varint};
use sha2::{Digest, Sha256};

/// tx_id = SHA-256("meshpay/tx_id/v1" || counter (u64 big endian) || SHA-256(body bytes without
/// the tx_id field)). The hash covers the exact bytes carried: the tx_id field is cut out at wire
/// level, every other byte (including unknown fields and field order) is kept as is.
pub fn derive_tx_id(counter: u64, body_bytes: &[u8]) -> Result<Vec<u8>, String> {
  let without_tx_id = strip_tx_id_field(body_bytes)?;
  let mut hasher = Sha256::new();
  hasher.update(b"meshpay/tx_id/v1");
  hasher.update(counter.to_be_bytes());
  hasher.update(Sha256::digest(without_tx_id));
  Ok(hasher.finalize().to_vec())
}

/// `body_bytes` with every `tx_id` field (number 4) removed, plus the field appended with `tx_id`.
pub fn body_with_tx_id(body_bytes: &[u8], tx_id: &[u8]) -> Result<Vec<u8>, String> {
  let mut bytes = strip_tx_id_field(body_bytes)?;
  bytes.push(4 << 3 | 2);
  encode_varint(tx_id.len() as u64, &mut bytes);
  bytes.extend_from_slice(tx_id);
  Ok(bytes)
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
      2 => decode_varint(&mut rest).map_err(|_| malformed())? as usize,
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

/// Message the payer signs: domain prefix plus the exact body bytes.
pub fn spend_signing_message(body_bytes: &[u8]) -> Vec<u8> {
  [b"meshpay/spend/v1".as_slice(), body_bytes].concat()
}

/// Message the receiver signs in an `AckReceipt`.
pub fn ack_signing_message(tx_id: &[u8], spend_hash: &[u8]) -> Vec<u8> {
  [b"meshpay/ack/v1".as_slice(), tx_id, spend_hash].concat()
}
