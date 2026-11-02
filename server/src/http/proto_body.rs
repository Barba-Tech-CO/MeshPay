use crate::error::ApiError;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use prost::Message;

pub fn decode<M: Message + Default>(bytes: &[u8]) -> Result<M, ApiError> {
  M::decode(bytes).map_err(|e| ApiError::BadRequest(format!("protobuf: {e}")))
}

pub fn encode<M: Message>(message: &M) -> Response {
  (
    [(CONTENT_TYPE, "application/x-protobuf")],
    message.encode_to_vec(),
  )
    .into_response()
}

pub fn unix_now() -> Result<u64, ApiError> {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs())
    .map_err(|e| ApiError::Internal(format!("clock: {e}")))
}
