use crate::error::ApiError;

pub fn to_i64(value: u64) -> Result<i64, ApiError> {
  i64::try_from(value).map_err(|_| ApiError::Unprocessable("amount out of range".to_owned()))
}

/// For database values already constrained to be non-negative.
pub fn to_u64(value: i64) -> Result<u64, ApiError> {
  u64::try_from(value).map_err(|_| ApiError::Internal("negative stored value".to_owned()))
}
