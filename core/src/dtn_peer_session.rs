use crate::dtn_buffer::{DtnEngine, MAX_PER_CONNECTION};
use crate::dtn_ingest_outcome::IngestOutcome;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// One connected peer: accepts at most `MAX_PER_CONNECTION` envelope attempts (accepted or
/// rejected alike, PRD 8.1); further attempts are refused without touching the buffer.
pub struct DtnPeerSession {
  engine: Arc<DtnEngine>,
  attempts: AtomicUsize,
  origin_pair: Option<Vec<u8>>,
}

impl DtnPeerSession {
  /// No origin pair: envelopes are NOT counted toward any per-pair cap (uncapped). Use
  /// `with_origin_pair` for peer traffic.
  pub fn new(engine: Arc<DtnEngine>) -> Self {
    Self {
      engine,
      attempts: AtomicUsize::new(0),
      origin_pair: None,
    }
  }

  /// Session whose envelopes count toward the per-pair cap of `origin_pair`, which must be derived
  /// from the authenticated session, not from unauthenticated peer bytes.
  pub fn with_origin_pair(engine: Arc<DtnEngine>, origin_pair: Vec<u8>) -> Self {
    Self {
      origin_pair: Some(origin_pair),
      ..Self::new(engine)
    }
  }

  pub fn ingest(&self, envelope_bytes: &[u8], now_unix: u64) -> Result<IngestOutcome, String> {
    if self.attempts.fetch_add(1, Ordering::SeqCst) >= MAX_PER_CONNECTION {
      return Ok(IngestOutcome::SessionBudgetExceeded);
    }
    self
      .engine
      .ingest_from(envelope_bytes, now_unix, self.origin_pair.as_deref())
  }
}
