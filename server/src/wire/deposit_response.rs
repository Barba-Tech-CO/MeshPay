use crate::wire::equivocation_proof::EquivocationProof;
use crate::wire::redeem_status::RedeemStatus;
use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct DepositResponse {
  #[prost(enumeration = "RedeemStatus", tag = "1")]
  pub status: i32,
  #[prost(string, tag = "2")]
  pub detail: String,
  #[prost(message, optional, tag = "3")]
  pub proof: Option<EquivocationProof>,
  #[prost(uint64, tag = "4")]
  pub credited_units: u64,
}
