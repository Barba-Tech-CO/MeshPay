use prost::Message;

/// PRD 7.4 `ServerKeyCert`: `body` is a serialized `ServerKeyCertBody`, signed by the offline root.
#[derive(Clone, PartialEq, Message)]
pub struct ServerKeyCert {
  #[prost(bytes = "vec", tag = "1")]
  pub body: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub root_signature: Vec<u8>,
}
