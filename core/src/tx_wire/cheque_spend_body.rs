use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct ChequeSpendBody {
  #[prost(bytes = "vec", tag = "1")]
  pub cheque_id: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub payee_account_id: Vec<u8>,
  #[prost(uint64, tag = "3")]
  pub amount_units: u64,
  #[prost(bytes = "vec", tag = "4")]
  pub tx_id: Vec<u8>,
  #[prost(uint64, tag = "5")]
  pub counter: u64,
  #[prost(bytes = "vec", tag = "6")]
  pub prev_spend_hash: Vec<u8>,
  #[prost(uint64, tag = "7")]
  pub signed_at_unix: u64,
}
