use crate::tx_wire::cheque_spend::ChequeSpend;
use prost::Message;

/// Lab simplification: no `ChequeStub` (tag 1 stays reserved).
#[derive(Clone, PartialEq, Message)]
pub struct TxPropose {
  #[prost(message, optional, tag = "2")]
  pub spend: Option<ChequeSpend>,
}
