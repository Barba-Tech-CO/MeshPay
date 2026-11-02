use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct AckReceipt {
  #[prost(bytes = "vec", tag = "1")]
  pub tx_id: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub spend_hash: Vec<u8>,
  #[prost(bytes = "vec", tag = "3")]
  pub payee_signature: Vec<u8>,
}
