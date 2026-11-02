use crate::wire::cheque_stub::ChequeStub;
use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct IssueResponse {
  #[prost(bytes = "vec", tag = "1")]
  pub chequebook_id: Vec<u8>,
  #[prost(message, repeated, tag = "2")]
  pub stubs: Vec<ChequeStub>,
  #[prost(uint64, tag = "3")]
  pub reserve_units: u64,
}
