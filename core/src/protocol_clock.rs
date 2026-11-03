use crate::protocol_now::ProtocolNow;

/// Largest forward jump `observe` accepts over the current protocol "now": the PRD wall-clock
/// tolerance of 7 days. A larger signed time is refused so one forged or buggy value cannot push
/// the monotonic-only clock arbitrarily far ahead.
pub const MAX_FORWARD_JUMP_SECONDS: u64 = 7 * 86_400;

/// A server time (`t_srv`) whose signature has been verified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedServerTime(u64);

impl VerifiedServerTime {
  /// Only construct after the server signature over `t_srv_unix` is verified; every
  /// `ProtocolClock` input goes through this type so unverified values cannot reach the clock.
  pub fn from_verified_signature(t_srv_unix: u64) -> Self {
    Self(t_srv_unix)
  }
}

/// `observe` refused a signed time beyond `now + MAX_FORWARD_JUMP_SECONDS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForwardJumpTooLarge;

/// Protocol clock (PRD 4.3 item 5, 12.0 items 23 and 31): `t_srv` + monotonic time elapsed since
/// it was seen. Wall clock is never an input. Monotonic readings come from the native plugin
/// (`elapsedRealtimeNanos` on Android with `BOOT_COUNT`, `CLOCK_MONOTONIC_RAW` on iOS with no boot
/// counter, so `None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolClock {
  t_srv_unix: u64,
  monotonic_at_t_srv_nanos: u64,
  boot_count: Option<u64>,
}

impl ProtocolClock {
  pub fn new(t_srv: VerifiedServerTime, monotonic_nanos: u64, boot_count: Option<u64>) -> Self {
    Self {
      t_srv_unix: t_srv.0,
      monotonic_at_t_srv_nanos: monotonic_nanos,
      boot_count,
    }
  }

  /// Re-anchors only on a signed time newer than the current protocol "now"; anything older is
  /// ignored, so the clock never moves backwards and a reboot stays flagged until a newer `t_srv`.
  pub fn observe(
    self,
    signed: VerifiedServerTime,
    monotonic_nanos: u64,
    boot_count: Option<u64>,
  ) -> Result<Self, ForwardJumpTooLarge> {
    let current = self.now(monotonic_nanos, boot_count).unix;
    if signed.0 > current.saturating_add(MAX_FORWARD_JUMP_SECONDS) {
      return Err(ForwardJumpTooLarge);
    }
    Ok(if signed.0 > current {
      Self::new(signed, monotonic_nanos, boot_count)
    } else {
      self
    })
  }

  /// Different boot counter, or monotonic value below the anchor (iOS rule), is a reboot:
  /// "now" falls back to `t_srv` exactly (PRD 12.1 point 13).
  pub fn now(&self, monotonic_nanos: u64, boot_count: Option<u64>) -> ProtocolNow {
    let rebooted = boot_count != self.boot_count || monotonic_nanos < self.monotonic_at_t_srv_nanos;
    let elapsed_secs = if rebooted {
      0
    } else {
      (monotonic_nanos - self.monotonic_at_t_srv_nanos) / 1_000_000_000
    };
    ProtocolNow {
      unix: self.t_srv_unix.saturating_add(elapsed_secs),
      t_srv_unix: self.t_srv_unix,
      rebooted,
    }
  }
}
