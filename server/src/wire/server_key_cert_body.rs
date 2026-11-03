use crate::wire::server_key_role::ServerKeyRole;
use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct ServerKeyCertBody {
  #[prost(bytes = "vec", tag = "1")]
  pub key_id: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub pubkey: Vec<u8>,
  #[prost(enumeration = "ServerKeyRole", tag = "3")]
  pub role: i32,
  #[prost(uint64, tag = "4")]
  pub not_before_unix: u64,
  #[prost(uint64, tag = "5")]
  pub not_after_unix: u64,
}
