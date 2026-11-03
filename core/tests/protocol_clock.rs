use meshpay_core::cheque_acceptance::ChequeAcceptance;
use meshpay_core::protocol_clock::{
  ForwardJumpTooLarge, MAX_FORWARD_JUMP_SECONDS, ProtocolClock, VerifiedServerTime,
};

fn verified(t: u64) -> VerifiedServerTime {
  VerifiedServerTime::from_verified_signature(t)
}

const T_SRV: u64 = 1_900_000_000;
const E: u64 = 72 * 3600;
const SECOND: u64 = 1_000_000_000;
const WEEK_SECS: i64 = 7 * 86_400;

/// PV-SEC-06: wall clock +/- 7 days never makes an expired cheque acceptable. The clock has no
/// wall-clock input, so the skewed wall reading is only fed as the cheque expiry a forged-clock
/// device would compute; the protocol decision ignores it.
#[test]
fn wall_clock_skew_never_accepts_expired_cheques() {
  let clock = ProtocolClock::new(verified(T_SRV), 1_000 * SECOND, Some(3));
  for case in 0..100u64 {
    let elapsed = 3600 + case * 60;
    let now = clock.now(1_000 * SECOND + elapsed * SECOND, Some(3));
    assert_eq!(now.unix, T_SRV + elapsed);
    let expired_at = now.unix - 1 - case;
    for skew in [-WEEK_SECS, 0, WEEK_SECS] {
      let wall = now.unix.checked_add_signed(skew).unwrap();
      let after_wall = clock
        .observe(
          verified(wall.min(T_SRV)),
          1_000 * SECOND + elapsed * SECOND,
          Some(3),
        )
        .unwrap();
      let reading = after_wall.now(1_000 * SECOND + elapsed * SECOND, Some(3));
      assert_eq!(reading, now);
      assert!(ChequeAcceptance::check(expired_at, reading, Some(T_SRV), E).is_err());
    }
  }
}

#[test]
fn now_counts_monotonic_time_from_t_srv() {
  let clock = ProtocolClock::new(verified(T_SRV), 50 * SECOND, None);
  assert_eq!(clock.now(50 * SECOND, None).unix, T_SRV);
  assert_eq!(
    clock.now(50 * SECOND + 3600 * SECOND, None).unix,
    T_SRV + 3600
  );
  let fresher = clock
    .observe(verified(T_SRV + 7200), 60 * SECOND, None)
    .unwrap();
  assert_eq!(fresher.now(70 * SECOND, None).unix, T_SRV + 7210);
  let stale = clock
    .observe(verified(T_SRV - 10), 60 * SECOND, None)
    .unwrap();
  assert_eq!(stale, clock);
}

#[test]
fn reboot_by_boot_counter_falls_back_to_t_srv() {
  let clock = ProtocolClock::new(verified(T_SRV), 10 * SECOND, Some(5));
  let now = clock.now(999_999 * SECOND, Some(6));
  assert_eq!(now.unix, T_SRV);
  assert!(now.rebooted);
  let old_signed = clock.observe(verified(T_SRV), SECOND, Some(6)).unwrap();
  assert!(old_signed.now(2 * SECOND, Some(6)).rebooted);
  let fresh = clock.observe(verified(T_SRV + 1), SECOND, Some(6)).unwrap();
  assert!(!fresh.now(2 * SECOND, Some(6)).rebooted);
}

#[test]
fn monotonic_regression_is_a_reboot_on_ios() {
  let clock = ProtocolClock::new(verified(T_SRV), 500 * SECOND, None);
  let now = clock.now(499 * SECOND, None);
  assert_eq!(now.unix, T_SRV);
  assert!(now.rebooted);
}

#[test]
fn after_reboot_only_cheques_expiring_within_e_of_t_srv() {
  let now = ProtocolClock::new(verified(T_SRV), 10 * SECOND, Some(1)).now(SECOND, Some(1));
  assert!(ChequeAcceptance::check(T_SRV + E, now, Some(T_SRV), E).is_ok());
  assert!(ChequeAcceptance::check(T_SRV + E + 1, now, Some(T_SRV), E).is_err());
}

#[test]
fn snapshot_older_than_s_max_refuses_every_cheque() {
  let clock = ProtocolClock::new(verified(T_SRV), 0, Some(1));
  for case in 0..100u64 {
    let now = clock.now((E + 1 + case) * SECOND, Some(1));
    assert!(ChequeAcceptance::check(now.unix + 3600, now, Some(T_SRV), E).is_err());
  }
  let now = clock.now(E * SECOND, Some(1));
  assert!(ChequeAcceptance::check(now.unix + 3600, now, Some(T_SRV), E).is_ok());
  assert!(ChequeAcceptance::check(now.unix + 3600, now, None, E).is_err());
}

#[test]
fn expiry_e_outside_24_to_168_hours_is_rejected() {
  let now = ProtocolClock::new(verified(T_SRV), 0, None).now(0, None);
  assert!(ChequeAcceptance::check(T_SRV + 60, now, Some(T_SRV), 23 * 3600).is_err());
  assert!(ChequeAcceptance::check(T_SRV + 60, now, Some(T_SRV), 169 * 3600).is_err());
}

#[test]
fn forged_huge_server_time_is_refused_and_does_not_brick_the_clock() {
  let clock = ProtocolClock::new(verified(T_SRV), 0, None);
  assert_eq!(
    clock.observe(verified(u64::MAX), SECOND, None),
    Err(ForwardJumpTooLarge)
  );
  let over = T_SRV + 1 + MAX_FORWARD_JUMP_SECONDS;
  assert_eq!(
    clock.observe(verified(over), 0, None),
    Err(ForwardJumpTooLarge)
  );
  let edge = clock
    .observe(verified(T_SRV + MAX_FORWARD_JUMP_SECONDS), 0, None)
    .unwrap();
  assert_eq!(edge.now(0, None).unix, T_SRV + MAX_FORWARD_JUMP_SECONDS);
  assert_eq!(clock.now(SECOND, None).unix, T_SRV + 1);
}

#[test]
fn snapshot_issued_in_the_future_is_rejected() {
  let now = ProtocolClock::new(verified(T_SRV), 0, None).now(0, None);
  assert!(ChequeAcceptance::check(T_SRV + 60, now, Some(T_SRV + 60), E).is_ok());
  assert!(ChequeAcceptance::check(T_SRV + 3600, now, Some(T_SRV + 61), E).is_err());
}
