#[test]
fn core_version_comes_from_the_crate_manifest() {
  assert_eq!(meshpay_core::api::core_version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn ping_returns_the_next_counter_value() {
  assert_eq!(meshpay_core::api::ping(41), 42);
}
