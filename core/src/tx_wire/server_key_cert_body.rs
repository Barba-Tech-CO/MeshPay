use prost::Message;

/// PRD 7.4 `ServerKeyCertBody`. `role` is the raw `ServerKeyRole` value (ISSUER = 1).
#[derive(Clone, PartialEq, Message)]
pub struct ServerKeyCertBody {
  #[prost(bytes = "vec", tag = "1")]
  pub key_id: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub pubkey: Vec<u8>,
  #[prost(int32, tag = "3")]
  pub role: i32,
  #[prost(uint64, tag = "4")]
  pub not_before_unix: u64,
  #[prost(uint64, tag = "5")]
  pub not_after_unix: u64,
}
