use crate::error::ApiError;
use prost::Message;

/// Decodes a protobuf value this server stored itself; failure is an internal error.
pub fn decode_stored<M: Message + Default>(bytes: &[u8], what: &str) -> Result<M, ApiError> {
  M::decode(bytes).map_err(|e| ApiError::Internal(format!("stored {what}: {e}")))
}
