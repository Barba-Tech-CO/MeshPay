use crate::error::ApiError;
use crate::http::proto_body::{encode, unix_now};
use crate::ledger::challenge;
use crate::state::AppState;
use crate::wire::challenge_response::ChallengeResponse;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::header::CACHE_CONTROL;
use axum::response::Response;

/// Public, rate limited: issues a single-use registration nonce.
pub async fn get_challenge(State(state): State<AppState>) -> Result<Response, ApiError> {
  if !state
    .challenge_limiter
    .allow(state.config.challenge_limit_per_minute)
  {
    return Err(ApiError::TooManyRequests("challenge rate limit".to_owned()));
  }
  let (nonce, expires_at_unix) = challenge::issue(
    &state.pool,
    unix_now()?,
    state.config.attestation_challenge_ttl_seconds,
  )
  .await?;
  let mut response = encode(&ChallengeResponse {
    nonce,
    expires_at_unix,
  });
  // A nonce must never be served from a shared cache.
  response
    .headers_mut()
    .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
  Ok(response)
}
