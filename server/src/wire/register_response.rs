use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct RegisterResponse {
  #[prost(bytes = "vec", tag = "1")]
  pub account_id: Vec<u8>,
  #[prost(uint64, tag = "2")]
  pub epoch: u64,
  #[prost(bool, tag = "3")]
  pub created: bool,
  #[prost(uint64, tag = "4")]
  pub available_units: u64,
}
