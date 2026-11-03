//! Shamir over GF(2^8) via the `blahaj` crate (PRD 9.2). No hand-rolled field math.
//! Plain Shamir has no integrity: the combined secret is checked against the public
//! key of RK registered at onboarding.
use crate::error::RecoveryError;
use crate::params::THRESHOLD;
use blahaj::{Share, Sharks};
use p256::ecdsa::{SigningKey, VerifyingKey};
use std::collections::HashSet;

/// Splits the RK scalar into `n` shares (raw bytes: x then y).
pub fn split(rk: &SigningKey, n: usize) -> Vec<Vec<u8>> {
  let secret = rk.to_bytes();
  Sharks(THRESHOLD)
    .dealer(secret.as_slice())
    .take(n)
    .map(|share| Vec::from(&share))
    .collect()
}

/// Reconstructs RK from >= k shares and verifies it against the registered public key.
pub fn combine(shares: &[Vec<u8>], registered: &VerifyingKey) -> Result<SigningKey, RecoveryError> {
  let mut seen = HashSet::new();
  let mut parsed = Vec::new();
  for raw in shares {
    let share = Share::try_from(raw.as_slice()).map_err(|_| RecoveryError::BadShare)?;
    if !seen.insert(share.x.0) {
      return Err(RecoveryError::DuplicateShare);
    }
    parsed.push(share);
  }
  if parsed.len() < THRESHOLD as usize {
    return Err(RecoveryError::NotEnoughShares);
  }
  let secret = Sharks(THRESHOLD)
    .recover(&parsed)
    .map_err(|_| RecoveryError::NotEnoughShares)?;
  let key = SigningKey::from_slice(&secret).map_err(|_| RecoveryError::WrongSecret)?;
  if key.verifying_key() != registered {
    return Err(RecoveryError::WrongSecret);
  }
  Ok(key)
}
