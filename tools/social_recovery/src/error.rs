use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum RecoveryError {
  BadKey,
  BadSignature,
  BadShare,
  NotEnoughShares,
  DuplicateShare,
  WrongSecret,
  Decrypt,
  UnknownAccount,
  StaleEpoch,
  AlreadyPending,
  NoPending,
  Frozen,
  Blocked,
  RateLimited,
  TimeLockActive,
  WrongEpoch,
  UnknownCheque,
  AlreadyDeposited,
  Expired,
  InsufficientFunds,
  Revoked,
  NotPayee,
  NoAllowance,
  DuplicateCheque,
  Overflow,
  WrongContext,
}

impl fmt::Display for RecoveryError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{self:?}")
  }
}

impl std::error::Error for RecoveryError {}
