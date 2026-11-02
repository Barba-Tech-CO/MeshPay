#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum RedeemStatus {
  Unspecified = 0,
  Paid = 1,
  AlreadyRedeemedSameBody = 2,
  RejectedConflict = 3,
  RejectedExpired = 4,
  RejectedRevoked = 5,
  RejectedInvalid = 6,
}
