use crate::dtn_buffer::{DtnEngine, MAX_PER_CONNECTION};
use crate::dtn_ingest_outcome::IngestOutcome;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// One connected peer: accepts at most `MAX_PER_CONNECTION` envelope attempts (accepted or
/// rejected alike, PRD 8.1); further attempts are refused without touching the buffer.
pub struct DtnPeerSession {
  engine: Arc<DtnEngine>,
  attempts: AtomicUsize,
}

impl DtnPeerSession {
  pub fn new(engine: Arc<DtnEngine>) -> Self {
    Self {
      engine,
      attempts: AtomicUsize::new(0),
    }
  }

  pub fn ingest(&self, envelope_bytes: &[u8], now_unix: u64) -> Result<IngestOutcome, String> {
    if self.attempts.fetch_add(1, Ordering::SeqCst) >= MAX_PER_CONNECTION {
      return Ok(IngestOutcome::SessionBudgetExceeded);
    }
    self.engine.ingest(envelope_bytes, now_unix)
  }
}
