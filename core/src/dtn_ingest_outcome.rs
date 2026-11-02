/// Result of offering an envelope to the DTN buffer. Everything except `Stored` leaves the
/// buffer unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IngestOutcome {
  Stored,
  Duplicate,
  /// Buffer full and nothing evictable for this envelope; nothing was written.
  Refused,
  /// Global ingest rate limit reached.
  RateLimited,
  /// The connection already used its envelope budget.
  SessionBudgetExceeded,
  Oversize,
  Malformed,
  IdMismatch,
  HopsExceeded,
  Expired,
  FromFuture,
  /// Public proofs cannot be verified yet (PRD 8.1: keep only verified proofs).
  UnverifiedProofs,
}
