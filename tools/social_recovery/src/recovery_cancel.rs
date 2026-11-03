use crate::codec::{put, put_u64};
use crate::keys;
use crate::recovery_cert::RecoveryCert;
use p256::ecdsa::SigningKey;
use sha2::{Digest, Sha256};

/// PRD 9.5 item 2: signed by the payment key of the OLD epoch. The signature binds the hash of
/// the pending cert and the per-account request counter, so a cancel for one pending recovery
/// cannot be replayed against another. The ledger recomputes both from its own pending state.
pub struct RecoveryCancel {
  pub account_id: String,
  pub epoch: u64,
  pub signature: Vec<u8>,
}

impl RecoveryCancel {
  pub fn signed_bytes(
    account_id: &str,
    epoch: u64,
    pending_cert: &RecoveryCert,
    request_no: u64,
  ) -> Vec<u8> {
    let mut buf = Vec::new();
    put(&mut buf, b"recovery-cancel-v1");
    put(&mut buf, account_id.as_bytes());
    put_u64(&mut buf, epoch);
    put(
      &mut buf,
      Sha256::digest(pending_cert.signed_bytes()).as_slice(),
    );
    put_u64(&mut buf, request_no);
    buf
  }

  pub fn sign(
    payment_key: &SigningKey,
    account_id: &str,
    epoch: u64,
    pending_cert: &RecoveryCert,
    request_no: u64,
  ) -> Self {
    let message = Self::signed_bytes(account_id, epoch, pending_cert, request_no);
    Self {
      account_id: account_id.to_owned(),
      epoch,
      signature: keys::sign(payment_key, &message),
    }
  }
}
