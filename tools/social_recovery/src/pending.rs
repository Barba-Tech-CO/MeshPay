use crate::recovery_cert::RecoveryCert;

pub struct Pending {
  pub cert: RecoveryCert,
  pub activates_at: u64,
  /// Per-account request counter at submission; bound into `RecoveryCancel`.
  pub request_no: u64,
}
