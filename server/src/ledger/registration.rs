use crate::crypto::p256_keys::{key_id, verify};
use crate::crypto::signing_messages::register_message;
use crate::error::ApiError;
use crate::ledger::units::to_u64;
use crate::ledger::{audit, balance};
use crate::state::AppState;
use crate::wire::attestation_level::AttestationLevel;
use crate::wire::register_request::RegisterRequest;
use crate::wire::register_response::RegisterResponse;

/// Registers a P-256 key. LAB ONLY: every new account receives the initial grant stored in
/// `supply` (a faucet), so this endpoint is rate limited per process and the real-value server
/// must not expose it.
/// The account id derives from the key, so repeating the call is idempotent and the grant is
/// booked once.
pub async fn register(
  state: &AppState,
  request: RegisterRequest,
) -> Result<RegisterResponse, ApiError> {
  if request.attestation_level != AttestationLevel::LabSoftware as i32 {
    return Err(ApiError::Unprocessable(
      "attestation verification is not implemented; only LAB_SOFTWARE".to_owned(),
    ));
  }
  if !state.config.lab_mode {
    return Err(ApiError::Forbidden(
      "software attestation refused outside lab_mode (PV-SEC-01)".to_owned(),
    ));
  }
  // Bad proof and a key-bound conflict give the same answer, with no verifier detail.
  let refused = || ApiError::Forbidden("registration refused".to_owned());
  verify(
    &request.public_key,
    &register_message(&request.public_key),
    &request.signature,
  )
  .map_err(|_| refused())?;
  // After the proof check, so junk requests cannot exhaust the faucet budget.
  if !state
    .register_limiter
    .allow(state.config.register_limit_per_minute)
  {
    return Err(ApiError::TooManyRequests(
      "registration rate limit".to_owned(),
    ));
  }

  let account_id = key_id(&request.public_key);
  let mut tx = state.pool.begin().await?;
  // Creates the account; the database trigger books the stored initial grant.
  let created: bool = sqlx::query_scalar("SELECT post_register($1)")
    .bind(&request.public_key)
    .fetch_one(&mut *tx)
    .await?;
  if created {
    audit::append(&mut tx, "ACCOUNT_REGISTERED", &account_id).await?;
  }
  let (stored_key, epoch): (Vec<u8>, i64) =
    sqlx::query_as("SELECT public_key, epoch FROM accounts WHERE account_id = $1")
      .bind(&account_id)
      .fetch_one(&mut *tx)
      .await?;
  if stored_key != request.public_key {
    return Err(refused());
  }
  tx.commit().await?;

  let current = balance::read_balance(&state.pool, &account_id).await?;
  Ok(RegisterResponse {
    account_id,
    epoch: to_u64(epoch)?,
    created,
    available_units: to_u64(current.available)?,
  })
}
