use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct ChallengeResponse {
  #[prost(bytes = "vec", tag = "1")]
  pub nonce: Vec<u8>,
  #[prost(uint64, tag = "2")]
  pub expires_at_unix: u64,
}
