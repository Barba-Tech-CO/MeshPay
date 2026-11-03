use crate::tx_wire::cheque_spend::ChequeSpend;
use crate::tx_wire::cheque_stub::ChequeStub;
use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct TxPropose {
  /// Server-issued stub; carries the cheque `expires_at_unix` the receiver checks.
  #[prost(message, optional, tag = "1")]
  pub stub: Option<ChequeStub>,
  #[prost(message, optional, tag = "2")]
  pub spend: Option<ChequeSpend>,
}
