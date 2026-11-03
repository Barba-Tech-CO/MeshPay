use crate::error::RecoveryError;
use p256::ecdsa::{
  Signature, SigningKey, VerifyingKey,
  signature::{Signer, Verifier},
};

pub fn random_signing_key() -> SigningKey {
  loop {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).expect("os randomness");
    if let Ok(key) = SigningKey::from_slice(&seed) {
      return key;
    }
  }
}

pub fn sign(key: &SigningKey, message: &[u8]) -> Vec<u8> {
  let signature: Signature = key.sign(message);
  signature.to_bytes().to_vec()
}

pub fn verify(key: &VerifyingKey, message: &[u8], signature: &[u8]) -> Result<(), RecoveryError> {
  let signature = Signature::from_slice(signature).map_err(|_| RecoveryError::BadSignature)?;
  key
    .verify(message, &signature)
    .map_err(|_| RecoveryError::BadSignature)
}
