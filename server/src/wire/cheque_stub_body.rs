use crate::wire::attestation_level::AttestationLevel;
use prost::Message;

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
  #[prost(enumeration = "AttestationLevel", tag = "8")]
  pub attestation_level: i32,
  #[prost(bool, tag = "9")]
  pub single_use_key: bool,
  #[prost(bytes = "vec", tag = "10")]
  pub server_key_id: Vec<u8>,
}
