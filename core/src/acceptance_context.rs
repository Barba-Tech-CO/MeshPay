use crate::protocol_now::ProtocolNow;

/// Receiver time inputs for one incoming cheque (PRD 4.3 items 3 and 5): a protocol clock reading
/// and the held revocation snapshot, both supplied by the caller, never the wall clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceptanceContext {
  pub now: ProtocolNow,
  /// `issued_at_unix` of the latest snapshot whose SNAPSHOT signature was verified; `None` denies.
  pub snapshot_issued_at_unix: Option<u64>,
  /// `E`; `S_max` = `E`.
  pub expiry_secs: u64,
}
