use crate::wire::cheque_spend::ChequeSpend;
use crate::wire::cheque_stub::ChequeStub;
use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct DepositRequest {
  #[prost(message, optional, tag = "1")]
  pub stub: Option<ChequeStub>,
  #[prost(message, optional, tag = "2")]
  pub spend: Option<ChequeSpend>,
}
