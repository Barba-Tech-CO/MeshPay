use crate::attestation::attestation_error::AttestationError;
use crate::error::ApiError;
use crate::ledger::units::to_i64;
use sqlx::{PgConnection, PgPool};

/// Stores a fresh random nonce valid for `ttl_seconds`; also drops expired ones.
pub async fn issue(
  pool: &PgPool,
  now_unix: u64,
  ttl_seconds: u64,
) -> Result<(Vec<u8>, u64), ApiError> {
  let mut nonce = vec![0u8; 32];
  getrandom::fill(&mut nonce).map_err(|e| ApiError::Internal(format!("random: {e}")))?;
  let expires_at = now_unix + ttl_seconds;
  sqlx::query("DELETE FROM challenges WHERE expires_at_unix < $1")
    .bind(to_i64(now_unix)?)
    .execute(pool)
    .await?;
  sqlx::query("INSERT INTO challenges (nonce, expires_at_unix) VALUES ($1, $2)")
    .bind(&nonce)
    .bind(to_i64(expires_at)?)
    .execute(pool)
    .await?;
  Ok((nonce, expires_at))
}

/// Single use: the row is deleted whether or not it was still valid. The outer error is a
/// database failure (500); the inner one is a refusal.
pub async fn consume(
  connection: &mut PgConnection,
  nonce: &[u8],
  now_unix: i64,
) -> Result<Result<(), AttestationError>, ApiError> {
  let expires_at: Option<i64> =
    sqlx::query_scalar("DELETE FROM challenges WHERE nonce = $1 RETURNING expires_at_unix")
      .bind(nonce)
      .fetch_optional(connection)
      .await?;
  Ok(match expires_at {
    None => Err(AttestationError::ChallengeUnknown),
    Some(at) if at < now_unix => Err(AttestationError::ChallengeExpired),
    Some(_) => Ok(()),
  })
}
