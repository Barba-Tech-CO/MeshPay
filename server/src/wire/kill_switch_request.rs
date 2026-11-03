use crate::wire::server_key_cert::ServerKeyCert;
use prost::Message;

/// Lab control body: a signed `RevocationSnapshot` plus the `ServerKeyCert` of its SNAPSHOT
/// signer (absent when the root signed it).
#[derive(Clone, PartialEq, Message)]
pub struct KillSwitchRequest {
  #[prost(bytes = "vec", tag = "1")]
  pub snapshot: Vec<u8>,
  #[prost(message, optional, tag = "2")]
  pub signer_cert: Option<ServerKeyCert>,
}
