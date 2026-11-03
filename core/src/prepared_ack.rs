/// External ack flow, phase 1: nothing is written until `finish_ack`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedAck {
  pub tx_id: Vec<u8>,
  /// Exact bytes to sign (`meshpay/ack/v1` || tx_id || spend_hash).
  pub message: Vec<u8>,
  /// Stored `AckReceipt` when already acknowledged: skip signing.
  pub existing_ack: Option<Vec<u8>>,
}
