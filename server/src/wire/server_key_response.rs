use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct ServerKeyResponse {
  #[prost(bytes = "vec", tag = "1")]
  pub public_key: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub key_id: Vec<u8>,
}
