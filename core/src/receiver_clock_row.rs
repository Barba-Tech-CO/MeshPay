/// Persisted receiver protocol-clock state (PRD 4.3 item 5).
/// - `t_srv_unix`: largest verified snapshot `issued_at_unix` ever installed.
/// - `anchor_unix`, `monotonic_nanos`, `boot_count`: "now" = `anchor_unix` + monotonic elapsed
///   since `monotonic_nanos` in boot `boot_count`. Equals `t_srv_unix` at install; after a
///   detected reboot it moves to `max_now_unix` and the new boot's monotonic reading.
/// - `max_now_unix`: largest protocol "now" ever computed, a floor so "now" never moves back.
/// - `rebooted`: sticky reboot seen since `t_srv_unix` was installed; cleared only by a newer
///   snapshot, so the `expires_at - t_srv <= E` bound cannot be lifted by uptime catching up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiverClockRow {
  pub t_srv_unix: u64,
  pub anchor_unix: u64,
  pub monotonic_nanos: u64,
  pub boot_count: Option<u64>,
  pub max_now_unix: u64,
  pub rebooted: bool,
}
