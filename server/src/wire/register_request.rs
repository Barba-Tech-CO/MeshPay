use crate::wire::attestation_level::AttestationLevel;
use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct RegisterRequest {
  #[prost(bytes = "vec", tag = "1")]
  pub public_key: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub signature: Vec<u8>,
  #[prost(enumeration = "AttestationLevel", tag = "3")]
  /// Client claim only; the stored level comes from verifying `attestation_chain`.
  pub attestation_level: i32,
  /// Android Key Attestation certificates, DER, leaf first.
  #[prost(bytes = "vec", repeated, tag = "4")]
  pub attestation_chain: Vec<Vec<u8>>,
  /// Nonce from `GET /v1/challenge`; must equal the attestation challenge in the leaf.
  #[prost(bytes = "vec", tag = "5")]
  pub challenge: Vec<u8>,
}
