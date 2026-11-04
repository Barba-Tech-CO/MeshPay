use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct ServerKeyResponse {
  #[prost(bytes = "vec", tag = "1")]
  pub public_key: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub key_id: Vec<u8>,
  /// Offline root public key (compressed SEC1) every cert chains to.
  #[prost(bytes = "vec", tag = "3")]
  pub root_public_key: Vec<u8>,
  /// Encoded ISSUER `ServerKeyCert` of `public_key` (core `IssuerTrust::new`).
  #[prost(bytes = "vec", tag = "4")]
  pub issuer_cert: Vec<u8>,
  /// Encoded SNAPSHOT `ServerKeyCert` of the key signing `/v1/revocation-snapshot`.
  #[prost(bytes = "vec", tag = "5")]
  pub snapshot_cert: Vec<u8>,
}
