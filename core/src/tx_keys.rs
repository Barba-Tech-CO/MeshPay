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
  signing_key
    .verifying_key()
    .to_sec1_point(true)
    .as_bytes()
    .to_vec()
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
