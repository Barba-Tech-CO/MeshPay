use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct ChequeSpend {
  /// Serialized `ChequeSpendBody`; signed byte for byte.
  #[prost(bytes = "vec", tag = "1")]
  pub body: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub payer_signature: Vec<u8>,
}
