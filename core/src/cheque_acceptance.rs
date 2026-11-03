use crate::protocol_now::ProtocolNow;

/// Receiver-side time checks for an incoming cheque (PRD 4.3 items 3 and 5, 12.0 items 22-23),
/// judged only by the protocol clock.
pub struct ChequeAcceptance;

impl ChequeAcceptance {
  pub const MIN_EXPIRY_SECS: u64 = 24 * 3600;
  pub const MAX_EXPIRY_SECS: u64 = 168 * 3600;
  /// A held snapshot may be stamped at most this far ahead of the protocol "now".
  pub const SNAPSHOT_FUTURE_SKEW_SECS: u64 = 60;

  /// `expiry_secs` is `E`; `S_max` = `E`. `snapshot_issued_at_unix` is the held
  /// `RevocationSnapshot.issued_at_unix` (`None` = no snapshot, default-deny).
  pub fn check(
    expires_at_unix: u64,
    now: ProtocolNow,
    snapshot_issued_at_unix: Option<u64>,
    expiry_secs: u64,
  ) -> Result<(), String> {
    if !(Self::MIN_EXPIRY_SECS..=Self::MAX_EXPIRY_SECS).contains(&expiry_secs) {
      return Err("expiry E must be 24 to 168 hours".to_owned());
    }
    let snapshot_issued = snapshot_issued_at_unix.ok_or("no revocation snapshot")?;
    if snapshot_issued > now.unix.saturating_add(Self::SNAPSHOT_FUTURE_SKEW_SECS) {
      return Err("revocation snapshot issued in the future".to_owned());
    }
    if now.unix.saturating_sub(snapshot_issued) > expiry_secs {
      return Err("revocation snapshot older than S_max".to_owned());
    }
    if now.unix >= expires_at_unix {
      return Err("cheque expired".to_owned());
    }
    if now.rebooted && expires_at_unix.saturating_sub(now.t_srv_unix) > expiry_secs {
      return Err("after reboot only cheques expiring within E of t_srv are accepted".to_owned());
    }
    Ok(())
  }
}
