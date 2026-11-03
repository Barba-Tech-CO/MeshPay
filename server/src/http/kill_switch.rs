use crate::error::ApiError;
use crate::http::proto_body::{decode, unix_now};
use crate::ledger::kill_switch;
use crate::state::AppState;
use crate::wire::kill_switch_request::KillSwitchRequest;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;

/// Junk is rate limited before any DB work. A snapshot with a valid root signature skips the
/// limit (one cheap ECDSA check), so anonymous traffic cannot lock out a real revocation.
pub async fn apply(State(state): State<AppState>, body: Bytes) -> Result<StatusCode, ApiError> {
  let parsed = decode::<KillSwitchRequest>(&body);
  let root_signed = parsed
    .as_ref()
    .is_ok_and(|request| kill_switch::is_root_signed(&state, request));
  if !root_signed
    && !state
      .control_limiter
      .allow(state.config.control_limit_per_minute)
  {
    return Err(ApiError::TooManyRequests("control rate limit".to_owned()));
  }
  kill_switch::apply(&state, parsed?, unix_now()?).await?;
  Ok(StatusCode::NO_CONTENT)
}
