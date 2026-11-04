//! Server-built `RevocationSnapshot` (PRD 14.2) for receivers: current revocations and freeze,
//! `issued_at_unix` from the server clock, signed by the SNAPSHOT key. Encoded exactly as core
//! `install_snapshot` / `ingest_snapshot` decode it.
use crate::crypto::p256_keys::{key_id, sign};
use crate::crypto::signing_messages::snapshot_signing_message;
use crate::error::ApiError;
use crate::ledger::server_keys;
use crate::state::AppState;
use crate::wire::revocation_snapshot::RevocationSnapshot;
use prost::Message;

pub async fn build_signed(state: &AppState, now_unix: u64) -> Result<Vec<u8>, ApiError> {
  let mut connection = state.pool.acquire().await?;
  let revoked_server_key_ids: Vec<Vec<u8>> =
    sqlx::query_scalar("SELECT key_id FROM revoked_server_keys ORDER BY key_id")
      .fetch_all(&mut *connection)
      .await?;
  let issuance_frozen = server_keys::issuance_frozen(&mut connection).await?;
  let now = i64::try_from(now_unix).map_err(|_| ApiError::Internal("clock overflow".to_owned()))?;
  let version: i64 = sqlx::query_scalar("SELECT next_snapshot_version($1)")
    .bind(now)
    .fetch_one(&mut *connection)
    .await?;
  let mut snapshot = RevocationSnapshot {
    version: u64::try_from(version)
      .map_err(|_| ApiError::Internal("negative snapshot version".to_owned()))?,
    issued_at_unix: now_unix,
    revoked_server_key_ids,
    issuance_frozen,
    signer_key_id: key_id(&state.snapshot_public_key()),
    ..Default::default()
  };
  snapshot.server_signature = sign(
    &state.snapshot_key,
    &snapshot_signing_message(&snapshot.encode_to_vec()),
  );
  Ok(snapshot.encode_to_vec())
}
