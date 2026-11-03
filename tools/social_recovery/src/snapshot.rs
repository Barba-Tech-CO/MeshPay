use crate::cheque::Cheque;
use crate::codec::{put, put_u64};
use crate::error::RecoveryError;
use crate::keys;
use p256::ecdsa::{SigningKey, VerifyingKey};
use std::collections::BTreeMap;

/// Non-sensitive metadata (PRD 9.2) signed by the server key: current epoch and payment public
/// key (SEC1) per account, plus `pending_recovery_accounts` (PRD 9.5 item 2). Receivers verify
/// the signature before trusting any field. Freshness is not modelled.
pub struct Snapshot {
  pub accounts: BTreeMap<String, (u64, Vec<u8>)>,
  pub pending_recovery_accounts: Vec<String>,
  pub signature: Vec<u8>,
}

impl Snapshot {
  pub fn signed_bytes(&self) -> Vec<u8> {
    let mut buf = Vec::new();
    put(&mut buf, b"snapshot-v1");
    for (id, (epoch, key)) in &self.accounts {
      put(&mut buf, id.as_bytes());
      put_u64(&mut buf, *epoch);
      put(&mut buf, key);
    }
    put(&mut buf, b"pending");
    for id in &self.pending_recovery_accounts {
      put(&mut buf, id.as_bytes());
    }
    buf
  }

  pub fn sign(mut self, server_key: &SigningKey) -> Self {
    self.signature = keys::sign(server_key, &self.signed_bytes());
    self
  }

  /// Receiver check (PV-REC-03): authentic snapshot, current epoch, valid cheque signature
  /// under the epoch key, not expired.
  pub fn accepts(
    &self,
    cheque: &Cheque,
    server_public: &VerifyingKey,
    now: u64,
  ) -> Result<(), RecoveryError> {
    keys::verify(server_public, &self.signed_bytes(), &self.signature)?;
    let (current, key) = self
      .accounts
      .get(&cheque.payer_account)
      .ok_or(RecoveryError::UnknownAccount)?;
    if cheque.epoch < *current {
      return Err(RecoveryError::Revoked);
    }
    if cheque.epoch > *current {
      return Err(RecoveryError::WrongEpoch);
    }
    let key = VerifyingKey::from_sec1_bytes(key).map_err(|_| RecoveryError::BadKey)?;
    keys::verify(&key, &cheque.signed_bytes(), &cheque.signature)?;
    if now > cheque.expiry {
      return Err(RecoveryError::Expired);
    }
    Ok(())
  }
}
