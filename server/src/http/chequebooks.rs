use crate::error::ApiError;
use crate::http::proto_body::{decode, encode, unix_now};
use crate::ledger::issuance;
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::State;
use axum::response::Response;

pub async fn issue(State(state): State<AppState>, body: Bytes) -> Result<Response, ApiError> {
  Ok(encode(
    &issuance::issue(&state, decode(&body)?, unix_now()?).await?,
  ))
}
