use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct IssueRequestBody {
  #[prost(bytes = "vec", tag = "1")]
  pub account_id: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub request_id: Vec<u8>,
  #[prost(uint32, tag = "3")]
  pub cheque_count: u32,
  #[prost(uint64, tag = "4")]
  pub max_amount_units: u64,
  #[prost(uint64, tag = "5")]
  pub ttl_seconds: u64,
}
