use prost::Message;

/// PRD section 10 `ChequeStubBody`, signed by the server ISSUER key. `attestation_level` is the
/// raw `AttestationLevel` enum value (not mirrored yet; wire compatible).
#[derive(Clone, PartialEq, Message)]
pub struct ChequeStubBody {
  #[prost(bytes = "vec", tag = "1")]
  pub cheque_id: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  pub chequebook_id: Vec<u8>,
  #[prost(bytes = "vec", tag = "3")]
  pub payer_account_id: Vec<u8>,
  #[prost(uint64, tag = "4")]
  pub payer_epoch: u64,
  #[prost(bytes = "vec", tag = "5")]
  pub signer_pubkey: Vec<u8>,
  #[prost(uint64, tag = "6")]
  pub max_amount_units: u64,
  #[prost(uint64, tag = "7")]
  pub expires_at_unix: u64,
  #[prost(int32, tag = "8")]
  pub attestation_level: i32,
  #[prost(bool, tag = "9")]
  pub single_use_key: bool,
  #[prost(bytes = "vec", tag = "10")]
  pub server_key_id: Vec<u8>,
}
