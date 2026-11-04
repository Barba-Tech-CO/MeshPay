//! LAB ONLY: software P-256 keys (PRD phase 3, TEST unit, no real value).
use p256::ecdsa::{
  Signature, SigningKey, VerifyingKey,
  signature::{Signer, Verifier},
};

pub fn signing_key_from_seed(seed: &[u8]) -> Result<SigningKey, String> {
  SigningKey::from_slice(seed).map_err(|error| format!("signing key: {error}"))
}

pub fn verifying_key_from_sec1(bytes: &[u8]) -> Result<VerifyingKey, String> {
  VerifyingKey::from_sec1_bytes(bytes).map_err(|error| format!("verifying key: {error}"))
}

/// Compressed SEC1 (33 bytes), the PRD public key format.
pub fn compressed_public_key(signing_key: &SigningKey) -> Vec<u8> {
  compressed_public_key_of(signing_key.verifying_key())
}

/// Compressed SEC1 (33 bytes) of a public key.
pub fn compressed_public_key_of(verifying_key: &VerifyingKey) -> Vec<u8> {
  verifying_key.to_sec1_point(true).as_bytes().to_vec()
}

/// Raw 64-byte ECDSA signature (deterministic, RFC 6979).
pub fn sign(signing_key: &SigningKey, message: &[u8]) -> Vec<u8> {
  let signature: Signature = signing_key.sign(message);
  signature.to_bytes().to_vec()
}

pub fn verify(
  verifying_key: &VerifyingKey,
  message: &[u8],
  signature: &[u8],
) -> Result<(), String> {
  let signature =
    Signature::from_slice(signature).map_err(|error| format!("signature: {error}"))?;
  verifying_key
    .verify(message, &signature)
    .map_err(|_| "signature verification failed".to_owned())
}

/// Verifies `signature` (raw 64 bytes or ASN.1 DER, which the Android Keystore returns) and
/// returns its raw low-S form. Raw is tried first, then DER; an input that parses as raw but does
/// not verify (a 64-byte DER) is still retried as DER.
pub fn verify_normalized(
  verifying_key: &VerifyingKey,
  message: &[u8],
  signature: &[u8],
) -> Result<Vec<u8>, String> {
  let mut parsed_any = false;
  for candidate in [
    Signature::from_slice(signature),
    Signature::from_der(signature),
  ]
  .into_iter()
  .flatten()
  {
    parsed_any = true;
    let low_s = candidate.normalize_s();
    if verifying_key.verify(message, &low_s).is_ok() {
      return Ok(low_s.to_bytes().to_vec());
    }
  }
  Err(if parsed_any {
    "signature verification failed".to_owned()
  } else {
    "signature: malformed".to_owned()
  })
}
