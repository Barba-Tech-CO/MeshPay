/// One reading of the protocol clock (PRD 4.3 item 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolNow {
  pub unix: u64,
  /// Largest server-signed `issued_at_unix` seen (`t_srv`).
  pub t_srv_unix: u64,
  /// Reboot detected since `t_srv` was anchored and no newer `t_srv` arrived: `unix == t_srv_unix`.
  pub rebooted: bool,
}
