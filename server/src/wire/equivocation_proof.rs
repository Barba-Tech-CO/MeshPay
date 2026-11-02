use crate::wire::cheque_spend::ChequeSpend;
use crate::wire::cheque_stub::ChequeStub;
use crate::wire::conflict_kind::ConflictKind;
use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct EquivocationProof {
  #[prost(bytes = "vec", tag = "1")]
  pub offender_account_id: Vec<u8>,
  #[prost(uint64, tag = "2")]
  pub epoch: u64,
  #[prost(enumeration = "ConflictKind", tag = "3")]
  pub conflict_kind: i32,
  #[prost(message, optional, tag = "4")]
  pub stub_1: Option<ChequeStub>,
  #[prost(message, optional, tag = "5")]
  pub spend_1: Option<ChequeSpend>,
  #[prost(message, optional, tag = "6")]
  pub stub_2: Option<ChequeStub>,
  #[prost(message, optional, tag = "7")]
  pub spend_2: Option<ChequeSpend>,
}
