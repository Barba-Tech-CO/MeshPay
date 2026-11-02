use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct IssueRequest {
  #[prost(bytes = "vec", tag = "1")]
  pub body: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub signature: Vec<u8>,
}
