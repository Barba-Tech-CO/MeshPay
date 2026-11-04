use crate::error::ApiError;
use crate::http::proto_body::unix_now;
use crate::ledger::revocation_snapshot;
use crate::state::AppState;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use std::time::{Duration, Instant};

/// Public: the current SNAPSHOT-signed `RevocationSnapshot` bytes.
pub async fn get_snapshot(State(state): State<AppState>) -> Result<Response, ApiError> {
  let bytes = cached_or_build(&state).await?;
  let mut response = ([(CONTENT_TYPE, "application/x-protobuf")], bytes).into_response();
  // A stale cached snapshot would hide a revocation.
  response
    .headers_mut()
    .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
  Ok(response)
}

/// Signed bytes from the last second, else a fresh snapshot (also cached).
// ponytail: the limiter guards only the DB read + signing (cache miss), so cached reads cannot
// be starved; the budget is still shared per endpoint, so one client can make misses 429 for
// others (per-IP buckets if it bites). Per-process cache, concurrent misses may each build once.
async fn cached_or_build(state: &AppState) -> Result<Vec<u8>, ApiError> {
  if let Ok(cache) = state.snapshot_cache.lock()
    && let Some((built, bytes)) = cache.as_ref()
    && built.elapsed() < Duration::from_secs(1)
  {
    return Ok(bytes.clone());
  }
  if !state
    .snapshot_limiter
    .allow(state.config.public_read_limit_per_minute)
  {
    return Err(ApiError::TooManyRequests(
      "public read rate limit".to_owned(),
    ));
  }
  let bytes = revocation_snapshot::build_signed(state, unix_now()?).await?;
  if let Ok(mut cache) = state.snapshot_cache.lock() {
    *cache = Some((Instant::now(), bytes.clone()));
  }
  Ok(bytes)
}
