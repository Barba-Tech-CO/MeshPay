use crate::crypto::p256_keys::verify;
use crate::crypto::signing_messages::balance_message;
use crate::error::ApiError;
use crate::http::proto_body::{decode, encode, unix_now};
use crate::ledger::units::to_u64;
use crate::ledger::{balance, registration};
use crate::state::AppState;
use crate::wire::balance_response::BalanceResponse;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;

pub async fn register(State(state): State<AppState>, body: Bytes) -> Result<Response, ApiError> {
  Ok(encode(
    &registration::register(&state, decode(&body)?).await?,
  ))
}

/// Balance of one account, readable only with a fresh request signed by that account's key:
/// `x-meshpay-timestamp` (unix seconds) and `x-meshpay-signature` (hex) over
/// "meshpay/balance/v1" || account_id || timestamp (u64 big endian). Unknown account, bad
/// signature and stale timestamp all answer the same 403.
pub async fn get_balance(
  State(state): State<AppState>,
  Path(account_hex): Path<String>,
  headers: HeaderMap,
) -> Result<Response, ApiError> {
  let denied = || ApiError::Forbidden("signed request required".to_owned());
  let header = |name: &str| {
    headers
      .get(name)
      .and_then(|v| v.to_str().ok())
      .ok_or_else(denied)
  };
  let account_id = hex::decode(account_hex).map_err(|_| denied())?;
  let timestamp: u64 = header("x-meshpay-timestamp")?
    .parse()
    .map_err(|_| denied())?;
  let signature = hex::decode(header("x-meshpay-signature")?).map_err(|_| denied())?;
  if unix_now()?.abs_diff(timestamp) > state.config.balance_auth_window_seconds {
    return Err(denied());
  }
  let public_key: Vec<u8> =
    sqlx::query_scalar("SELECT public_key FROM accounts WHERE account_id = $1")
      .bind(&account_id)
      .fetch_optional(&state.pool)
      .await?
      .ok_or_else(denied)?;
  let message = balance_message(&account_id, timestamp);
  verify(&public_key, &message, &signature).map_err(|_| denied())?;

  let current = balance::read_balance(&state.pool, &account_id).await?;
  Ok(encode(&BalanceResponse {
    available_units: to_u64(current.available)?,
    reserved_units: to_u64(current.reserved)?,
    frozen: current.frozen,
  }))
}
