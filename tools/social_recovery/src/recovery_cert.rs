use crate::codec::{put, put_u64};
use crate::keys;
use p256::ecdsa::SigningKey;

/// PRD 9.2: signed by RK. `attestation` is opaque here (hardware attestation not checkable).
#[derive(Clone)]
pub struct RecoveryCert {
  pub account_id: String,
  pub old_epoch: u64,
  pub new_epoch: u64,
  pub new_device_pubkey: Vec<u8>,
  pub attestation: Vec<u8>,
  pub signature: Vec<u8>,
}

impl RecoveryCert {
  pub fn signed_bytes(&self) -> Vec<u8> {
    let mut buf = Vec::new();
    put(&mut buf, b"recovery-cert-v1");
    put(&mut buf, self.account_id.as_bytes());
    put_u64(&mut buf, self.old_epoch);
    put_u64(&mut buf, self.new_epoch);
    put(&mut buf, &self.new_device_pubkey);
    put(&mut buf, &self.attestation);
    buf
  }

  pub fn sign(
    rk: &SigningKey,
    account_id: &str,
    old_epoch: u64,
    new_device_pubkey: Vec<u8>,
    attestation: Vec<u8>,
  ) -> Self {
    let mut cert = Self {
      account_id: account_id.to_owned(),
      old_epoch,
      new_epoch: old_epoch + 1,
      new_device_pubkey,
      attestation,
      signature: Vec::new(),
    };
    cert.signature = keys::sign(rk, &cert.signed_bytes());
    cert
  }
}
