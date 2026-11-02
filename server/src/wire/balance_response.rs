use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct BalanceResponse {
  #[prost(uint64, tag = "1")]
  pub available_units: u64,
  #[prost(uint64, tag = "2")]
  pub reserved_units: u64,
  #[prost(bool, tag = "3")]
  pub frozen: bool,
}
