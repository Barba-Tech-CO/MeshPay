use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct ChequeStub {
  /// Serialized `ChequeStubBody`; signed byte for byte by the server.
  #[prost(bytes = "vec", tag = "1")]
  pub body: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub server_signature: Vec<u8>,
}
