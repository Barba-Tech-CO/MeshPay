use crate::attestation::account_level::AccountLevel;
use crate::attestation::attestation_error::AttestationError;
use crate::attestation::chain_verifier::verify_chain;
use crate::crypto::p256_keys::{key_id, verify};
use crate::crypto::signing_messages::register_message;
use crate::error::ApiError;
use crate::http::proto_body::unix_now;
use crate::ledger::units::{to_i64, to_u64};
use crate::ledger::{audit, balance, challenge};
use crate::state::AppState;
use crate::wire::attestation_level::AttestationLevel;
use crate::wire::register_request::RegisterRequest;
use crate::wire::register_response::RegisterResponse;
use sqlx::PgConnection;

/// Registers a P-256 key. LAB ONLY: every new account receives the initial grant stored in
/// `supply` (a faucet), so this endpoint is rate limited per process and the real-value server
/// must not expose it.
/// The account id derives from the key, so repeating the call is idempotent and the grant is
/// booked once.
pub async fn register(
  state: &AppState,
  request: RegisterRequest,
) -> Result<RegisterResponse, ApiError> {
  // Bad proof, bad attestation and a key-bound conflict give the same answer, no verifier detail.
  let refused = || ApiError::Forbidden("registration refused".to_owned());
  // The client's `attestation_level` is a claim and is ignored: a chain means verification
  // decides; no chain is only valid as LAB_SOFTWARE in lab mode (PV-SEC-01).
  let attested = !request.attestation_chain.is_empty();
  if !attested
    && !(state.config.lab_mode && request.attestation_level == AttestationLevel::LabSoftware as i32)
  {
    return Err(refused());
  }
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
  let now = to_i64(unix_now()?)?;
  let mut tx = state.pool.begin().await?;
  let level = if attested {
    match attested_level(&mut tx, state, &request, now).await? {
      Ok(level) => level,
      Err(error) => {
        // The nonce is burned even on failure; commit that so a refused chain cannot be retried.
        tx.commit().await?;
        tracing::warn!(code = error.code(), "attestation refused");
        return Err(refused());
      }
    }
  } else {
    AccountLevel::LabSoftware
  };
  // Creates the account; the database trigger books the stored initial grant.
  let created: bool = sqlx::query_scalar("SELECT post_register($1, $2)")
    .bind(&request.public_key)
    .bind(level)
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

/// Burns the challenge, verifies the chain and applies the level policy. The outer error is a
/// server failure; the inner one is a refusal (callers log its code, the client sees one 403).
async fn attested_level(
  connection: &mut PgConnection,
  state: &AppState,
  request: &RegisterRequest,
  now: i64,
) -> Result<Result<AccountLevel, AttestationError>, ApiError> {
  let consumed = challenge::consume(connection, &request.challenge, now).await?;
  Ok(
    consumed
      .and_then(|()| {
        verify_chain(
          &request.attestation_chain,
          &state.trust_store,
          now,
          &request.public_key,
          &request.challenge,
        )
      })
      .and_then(|level| match level {
        // Software-level accounts never get the free initial grant outside the lab.
        AccountLevel::SoftwareAttested if !state.config.lab_mode => {
          Err(AttestationError::SoftwareLevelNotAllowed)
        }
        _ => Ok(level),
      }),
  )
}
