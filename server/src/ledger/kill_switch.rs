//! Kill switch (PRD 14.2): a `RevocationSnapshot` signed by a SNAPSHOT key (with its root-signed
//! `ServerKeyCert`) or by the root itself. Its `revoked_server_key_ids` and `issuance_frozen` are
//! applied append-only; replaying a snapshot changes nothing (no unrevoke, no unfreeze, no rows).
use crate::crypto::p256_keys::{KEY_ID_LEN, key_id, verify};
use crate::crypto::server_key_cert::{is_current, verify_cert};
use crate::crypto::signing_messages::snapshot_signing_message;
use crate::error::ApiError;
use crate::ledger::units::to_i64;
use crate::ledger::{audit, server_keys};
use crate::state::AppState;
use crate::wire::kill_switch_request::KillSwitchRequest;
use crate::wire::revocation_snapshot::RevocationSnapshot;
use crate::wire::server_key_role::ServerKeyRole;
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;

/// Accepted clock skew of a snapshot's `issued_at_unix`. Same 60 s as `issuance_freezes` CHECK.
const CLOCK_SKEW_SECONDS: u64 = 60;

fn denied(why: impl std::fmt::Display) -> ApiError {
  ApiError::forbidden(format!("snapshot refused: {why}"))
}

pub async fn apply(
  state: &AppState,
  request: KillSwitchRequest,
  now_unix: u64,
) -> Result<(), ApiError> {
  let snapshot = RevocationSnapshot::decode(request.snapshot.as_slice())
    .map_err(|e| ApiError::BadRequest(format!("snapshot: {e}")))?;
  // Only canonical bytes: otherwise padding (unknown fields) gives the same signed snapshot a
  // new hash and a new row on every replay.
  if snapshot.encode_to_vec() != request.snapshot {
    return Err(ApiError::BadRequest(
      "snapshot is not canonically encoded".to_owned(),
    ));
  }
  // A future snapshot would move the freeze point later.
  if snapshot.issued_at_unix > now_unix + CLOCK_SKEW_SECONDS {
    return Err(denied("issued in the future"));
  }
  if snapshot
    .revoked_server_key_ids
    .iter()
    .any(|id| id.len() != KEY_ID_LEN)
  {
    return Err(ApiError::BadRequest(
      "revoked_server_key_ids are 16 bytes".to_owned(),
    ));
  }
  let mut tx = state.pool.begin().await?;
  verify_signer(&mut tx, state, &request, &snapshot, now_unix).await?;
  let snapshot_hash = Sha256::digest(&request.snapshot).to_vec();
  revoke_keys(&mut tx, &snapshot.revoked_server_key_ids, &snapshot_hash).await?;
  if snapshot.issuance_frozen {
    record_freeze(&mut tx, snapshot.issued_at_unix, &snapshot_hash).await?;
  }
  tx.commit().await?;
  Ok(())
}

/// Root signature, or a current, unrevoked SNAPSHOT key certified by the root (cert stored).
async fn verify_signer(
  connection: &mut PgConnection,
  state: &AppState,
  request: &KillSwitchRequest,
  snapshot: &RevocationSnapshot,
  now_unix: u64,
) -> Result<(), ApiError> {
  let unsigned = RevocationSnapshot {
    server_signature: Vec::new(),
    ..snapshot.clone()
  };
  let message = snapshot_signing_message(&unsigned.encode_to_vec());
  if snapshot.signer_key_id == key_id(&state.root_public_key) {
    return verify(&state.root_public_key, &message, &snapshot.server_signature)
      .map_err(|_| denied("bad root signature"));
  }
  let cert = request
    .signer_cert
    .as_ref()
    .ok_or_else(|| denied("SNAPSHOT signer cert missing"))?;
  let body = verify_cert(cert, &state.root_public_key, ServerKeyRole::Snapshot).map_err(denied)?;
  if body.key_id != snapshot.signer_key_id || !is_current(&body, now_unix) {
    return Err(denied("signer cert expired or for another key"));
  }
  if server_keys::is_revoked(connection, &body.key_id).await? {
    return Err(denied("signer key revoked"));
  }
  verify(&body.pubkey, &message, &snapshot.server_signature)
    .map_err(|_| denied("bad SNAPSHOT signature"))?;
  server_keys::register_cert(
    connection,
    &body.key_id,
    ServerKeyRole::Snapshot,
    &cert.encode_to_vec(),
  )
  .await?;
  Ok(())
}

async fn revoke_keys(
  connection: &mut PgConnection,
  key_ids: &[Vec<u8>],
  snapshot_hash: &[u8],
) -> Result<(), ApiError> {
  for revoked in key_ids {
    let query = sqlx::query(
      "INSERT INTO revoked_server_keys (key_id, snapshot_hash) VALUES ($1, $2)
       ON CONFLICT (key_id) DO NOTHING",
    )
    .bind(revoked)
    .bind(snapshot_hash);
    insert_and_audit(connection, query, "SERVER_KEY_REVOKED", revoked).await?;
  }
  Ok(())
}

/// Freeze point = max(issued_at, db now - 60 s): a backdated snapshot cannot reach cheques issued
/// earlier than the skew window. Only a freeze earlier than every existing one adds a row.
async fn record_freeze(
  connection: &mut PgConnection,
  issued_at_unix: u64,
  snapshot_hash: &[u8],
) -> Result<(), ApiError> {
  let query = sqlx::query(
    "INSERT INTO issuance_freezes (snapshot_hash, frozen_at_unix)
     SELECT $1, point FROM (
       SELECT greatest($2, floor(extract(epoch FROM now()))::bigint - 60) AS point) p
     WHERE NOT EXISTS (SELECT 1 FROM issuance_freezes WHERE frozen_at_unix <= p.point)
     ON CONFLICT (snapshot_hash) DO NOTHING",
  )
  .bind(snapshot_hash)
  .bind(to_i64(issued_at_unix)?);
  insert_and_audit(connection, query, "ISSUANCE_FROZEN", snapshot_hash).await
}

/// Runs an idempotent insert and appends the audit event only when it added a row.
async fn insert_and_audit<'q>(
  connection: &mut PgConnection,
  query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
  event: &str,
  ref_id: &[u8],
) -> Result<(), ApiError> {
  if query.execute(&mut *connection).await?.rows_affected() == 1 {
    audit::append(connection, event, ref_id).await?;
  }
  Ok(())
}
