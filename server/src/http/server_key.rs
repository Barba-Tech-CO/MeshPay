use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};

/// Public, rate limited: issuer key, root public key and the ISSUER / SNAPSHOT certs.
pub async fn server_key(State(state): State<AppState>) -> Result<Response, ApiError> {
  if !state
    .server_key_limiter
    .allow(state.config.public_read_limit_per_minute)
  {
    return Err(ApiError::TooManyRequests(
      "public read rate limit".to_owned(),
    ));
  }
  Ok(
    (
      [(CONTENT_TYPE, "application/x-protobuf")],
      state.server_key_response.clone(),
    )
      .into_response(),
  )
}
