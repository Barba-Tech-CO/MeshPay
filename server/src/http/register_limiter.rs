use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Per-process fixed-window limit on registrations (the lab faucet). Not shared across replicas.
pub struct RegisterLimiter {
  window: Mutex<(Instant, u32)>,
}

impl RegisterLimiter {
  pub fn new() -> Self {
    Self {
      window: Mutex::new((Instant::now(), 0)),
    }
  }

  pub fn allow(&self, limit_per_minute: u32) -> bool {
    let Ok(mut window) = self.window.lock() else {
      return false;
    };
    if window.0.elapsed() >= Duration::from_secs(60) {
      *window = (Instant::now(), 0);
    }
    window.1 += 1;
    window.1 <= limit_per_minute
  }
}

impl Default for RegisterLimiter {
  fn default() -> Self {
    Self::new()
  }
}
