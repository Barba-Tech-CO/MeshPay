use crate::codec::{put, put_u64};
use crate::keys;
use p256::ecdsa::SigningKey;

/// Minimal cheque: enough to test epoch revocation and quarantine (PRD 9.3), not the full
/// ledger cheque format.
#[derive(Clone)]
pub struct Cheque {
  pub id: u64,
  pub payer_account: String,
  pub payee_account: String,
  pub epoch: u64,
  pub amount: u64,
  pub expiry: u64,
  pub signature: Vec<u8>,
}

impl Cheque {
  pub fn signed_bytes(&self) -> Vec<u8> {
    let mut buf = Vec::new();
    put(&mut buf, b"cheque-v1");
    put_u64(&mut buf, self.id);
    put(&mut buf, self.payer_account.as_bytes());
    put(&mut buf, self.payee_account.as_bytes());
    put_u64(&mut buf, self.epoch);
    put_u64(&mut buf, self.amount);
    put_u64(&mut buf, self.expiry);
    buf
  }

  pub fn sign(mut self, payment_key: &SigningKey) -> Self {
    self.signature = keys::sign(payment_key, &self.signed_bytes());
    self
  }
}
