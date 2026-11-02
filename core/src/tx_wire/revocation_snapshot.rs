use prost::Message;

/// PRD section 10 `RevocationSnapshot`. `new_proofs` (tag 7) is carried as opaque bytes
/// (serialized `EquivocationProof`s, not mirrored yet; wire compatible).
#[derive(Clone, PartialEq, Message)]
pub struct RevocationSnapshot {
  #[prost(uint64, tag = "1")]
  pub version: u64,
  #[prost(uint64, tag = "2")]
  pub issued_at_unix: u64,
  #[prost(bytes = "vec", repeated, tag = "3")]
  pub revoked_signer_key_ids: Vec<Vec<u8>>,
  #[prost(bytes = "vec", repeated, tag = "4")]
  pub revoked_account_epochs: Vec<Vec<u8>>,
  #[prost(bytes = "vec", repeated, tag = "5")]
  pub revoked_attestation_serials: Vec<Vec<u8>>,
  #[prost(bytes = "vec", repeated, tag = "6")]
  pub trusted_attestation_root_ids: Vec<Vec<u8>>,
  #[prost(bytes = "vec", repeated, tag = "7")]
  pub new_proofs: Vec<Vec<u8>>,
  #[prost(bytes = "vec", tag = "8")]
  pub server_signature: Vec<u8>,
  #[prost(bytes = "vec", repeated, tag = "9")]
  pub revoked_server_key_ids: Vec<Vec<u8>>,
  #[prost(bool, tag = "10")]
  pub issuance_frozen: bool,
  #[prost(bytes = "vec", tag = "11")]
  pub audit_head_hash: Vec<u8>,
  #[prost(bytes = "vec", repeated, tag = "12")]
  pub pending_recovery_accounts: Vec<Vec<u8>>,
  #[prost(bytes = "vec", tag = "13")]
  pub signer_key_id: Vec<u8>,
}
