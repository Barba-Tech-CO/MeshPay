/// Phase 1 of the external payer flow. Nothing is stored: a crash before `finish_propose` leaves no row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedPropose {
  /// Exact bytes to sign (`meshpay/spend/v1` || body incl. tx_id); ECDSA hashes with SHA-256.
  pub message: Vec<u8>,
  /// Serialized `ChequeSpendBody` carrying the derived tx_id; handed back to `finish_propose`.
  pub body: Vec<u8>,
  /// Serialized server `ChequeStub` sent alongside the spend.
  pub stub: Vec<u8>,
  pub counter: u64,
  /// Stored `TxPropose` when this counter and content were already proposed: skip signing.
  pub existing_propose: Option<Vec<u8>>,
}
