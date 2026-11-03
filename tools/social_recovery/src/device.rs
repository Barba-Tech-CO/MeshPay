use crate::cheque::Cheque;
use crate::error::RecoveryError;
use crate::keys;
use crate::recovery_cert::RecoveryCert;
use crate::shares;
use crate::snapshot::Snapshot;
use p256::ecdsa::{SigningKey, VerifyingKey};

/// Stand-in for a phone. `allowance` is the offline spending allowance (talon) issued by the
/// server; a freshly recovered device has none (PRD 9.4 step 2).
pub struct Device {
  pub account_id: String,
  pub epoch: u64,
  pub payment_key: SigningKey,
  allowance: u64,
  pub received: Vec<Cheque>,
}

impl Device {
  pub fn new(account_id: &str, epoch: u64) -> Self {
    Self {
      account_id: account_id.to_owned(),
      epoch,
      payment_key: keys::random_signing_key(),
      allowance: 0,
      received: Vec::new(),
    }
  }

  pub fn payment_public(&self) -> VerifyingKey {
    *self.payment_key.verifying_key()
  }

  /// PRD 9.3 steps 1-3 / 9.4 step 1: new payment key, rebuild RK from guardian shares
  /// (no network needed), sign the RecoveryCert.
  pub fn recover(
    account_id: &str,
    old_epoch: u64,
    opened_shares: &[Vec<u8>],
    rk_public: &VerifyingKey,
    attestation: Vec<u8>,
  ) -> Result<(Self, RecoveryCert), RecoveryError> {
    let rk = shares::combine(opened_shares, rk_public)?;
    let device = Self::new(account_id, old_epoch + 1);
    let public = device
      .payment_public()
      .to_sec1_point(true)
      .as_bytes()
      .to_vec();
    let cert = RecoveryCert::sign(&rk, account_id, old_epoch, public, attestation);
    Ok((device, cert))
  }

  pub fn grant_allowance(&mut self, amount: u64) {
    self.allowance += amount;
  }

  /// Offline spend: bounded by the server-issued allowance, so a recovered device cannot
  /// spend the old balance (PRD 9.4 step 2).
  pub fn spend_offline(&mut self, amount: u64) -> Result<(), RecoveryError> {
    if amount > self.allowance {
      return Err(RecoveryError::NoAllowance);
    }
    self.allowance -= amount;
    Ok(())
  }

  pub fn receive(
    &mut self,
    cheque: Cheque,
    snapshot: &Snapshot,
    server_public: &VerifyingKey,
    now: u64,
  ) -> Result<(), RecoveryError> {
    if cheque.payee_account != self.account_id {
      return Err(RecoveryError::NotPayee);
    }
    snapshot.accepts(&cheque, server_public, now)?;
    self.received.push(cheque);
    Ok(())
  }
}
