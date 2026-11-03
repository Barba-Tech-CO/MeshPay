use crate::error::ApiError;
use crate::http::proto_body::{decode, unix_now};
use crate::ledger::kill_switch;
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;

/// Rate limited before any signature work, so junk snapshots cannot burn CPU or DB time.
pub async fn apply(State(state): State<AppState>, body: Bytes) -> Result<StatusCode, ApiError> {
  if !state
    .control_limiter
    .allow(state.config.control_limit_per_minute)
  {
    return Err(ApiError::TooManyRequests("control rate limit".to_owned()));
  }
  kill_switch::apply(&state, decode(&body)?, unix_now()?).await?;
  Ok(StatusCode::NO_CONTENT)
}
