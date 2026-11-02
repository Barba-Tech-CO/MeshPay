use prost::Message;
use sha2::{Digest, Sha256};

/// PRD section 10 `SyncEnvelope` (DTN mule envelope).
#[derive(Clone, PartialEq, Message)]
pub struct SyncEnvelope {
  /// SHA-256 of `encrypted_server_batch`.
  #[prost(bytes = "vec", tag = "1")]
  pub envelope_id: Vec<u8>,
  #[prost(uint64, tag = "2")]
  pub created_at_unix: u64,
  /// Remaining hops; MVP maximum 4.
  #[prost(uint32, tag = "3")]
  pub ttl_hops: u32,
  #[prost(uint64, tag = "4")]
  pub known_revocation_version: u64,
  /// Serialized `EquivocationProof`s. LAB: no verifier yet, so the buffer rejects non-empty.
  #[prost(bytes = "vec", repeated, tag = "5")]
  pub public_proofs: Vec<Vec<u8>>,
  /// Opaque to every carrier (SealedBox X25519 to the server in the real protocol).
  #[prost(bytes = "vec", tag = "6")]
  pub encrypted_server_batch: Vec<u8>,
}

impl SyncEnvelope {
  pub fn new(
    encrypted_server_batch: Vec<u8>,
    created_at_unix: u64,
    ttl_hops: u32,
    known_revocation_version: u64,
  ) -> Self {
    Self {
      envelope_id: Sha256::digest(&encrypted_server_batch).to_vec(),
      created_at_unix,
      ttl_hops,
      known_revocation_version,
      public_proofs: Vec::new(),
      encrypted_server_batch,
    }
  }
}
