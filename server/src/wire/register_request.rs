use crate::wire::attestation_level::AttestationLevel;
use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct RegisterRequest {
  #[prost(bytes = "vec", tag = "1")]
  pub public_key: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub signature: Vec<u8>,
  #[prost(enumeration = "AttestationLevel", tag = "3")]
  pub attestation_level: i32,
}
