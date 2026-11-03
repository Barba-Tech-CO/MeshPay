//! LAB software P-256 keys; the server of real value refuses software attestation (PV-SEC-01).
use p256::ecdsa::{
  Signature, SigningKey, VerifyingKey,
  signature::{Signer, Verifier},
};
use sha2::{Digest, Sha256};

/// Length of an account id or server key id (`key_id`).
pub const KEY_ID_LEN: usize = 16;
/// Length of a compressed SEC1 P-256 public key.
pub const COMPRESSED_PUBLIC_KEY_LEN: usize = 33;

pub fn signing_key_from_seed(seed: &[u8]) -> Result<SigningKey, String> {
  SigningKey::from_slice(seed).map_err(|error| format!("signing key: {error}"))
}

/// Compressed SEC1 (33 bytes), the PRD public key format.
pub fn compressed_public_key(signing_key: &SigningKey) -> Vec<u8> {
  signing_key
    .verifying_key()
    .to_sec1_point(true)
    .as_bytes()
    .to_vec()
}

/// Raw 64-byte ECDSA signature.
pub fn sign(signing_key: &SigningKey, message: &[u8]) -> Vec<u8> {
  let signature: Signature = signing_key.sign(message);
  signature.to_bytes().to_vec()
}

pub fn verify(public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<(), String> {
  let key =
    VerifyingKey::from_sec1_bytes(public_key).map_err(|error| format!("public key: {error}"))?;
  let signature =
    Signature::from_slice(signature).map_err(|error| format!("signature: {error}"))?;
  key
    .verify(message, &signature)
    .map_err(|_| "signature verification failed".to_owned())
}

/// First 16 bytes of SHA-256 of the compressed public key: account id and server key id.
pub fn key_id(public_key: &[u8]) -> Vec<u8> {
  Sha256::digest(public_key)[..KEY_ID_LEN].to_vec()
}
