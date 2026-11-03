use crate::error::ApiError;
use crate::http::proto_body::{decode, encode};
use crate::ledger::deposit;
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::State;
use axum::response::Response;

pub async fn deposit(State(state): State<AppState>, body: Bytes) -> Result<Response, ApiError> {
  Ok(encode(&deposit::deposit(&state, decode(&body)?).await?))
}
