//! Server key checks for stubs (PRD 7.4, 14.2, PV-SEC-05): a stub counts only when its
//! `server_key_id` had a root-signed ISSUER `ServerKeyCert` current when its chequebook was
//! issued, the key is not revoked, the seal verifies, and the stub predates the issuance freeze.
use crate::crypto::p256_keys::verify;
use crate::crypto::server_key_cert::{CertError, is_current, verify_cert};
use crate::crypto::signing_messages::stub_seal_message;
use crate::error::ApiError;
use crate::ledger::stored::decode_stored;
use crate::ledger::units::to_u64;
use crate::state::AppState;
use crate::wire::cheque_stub::ChequeStub;
use crate::wire::server_key_cert::ServerKeyCert;
use crate::wire::server_key_role::ServerKeyRole;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;

/// Why a stub is not payable under its server key; mapped to a deposit status by the caller.
pub enum KeyRefusal {
  /// Unknown key, bad root signature, wrong role, lifetime or seal: not a valid stub.
  Invalid(&'static str),
  /// No cert of the key was current when the chequebook was issued.
  Expired,
  Revoked,
  IssuedAfterFreeze,
}

/// Stores a verified cert so later deposits can find it (idempotent per cert).
pub async fn register_cert(
  connection: &mut PgConnection,
  key_id: &[u8],
  role: ServerKeyRole,
  cert_bytes: &[u8],
) -> Result<(), sqlx::Error> {
  sqlx::query(
    "INSERT INTO server_key_certs (key_id, role, cert_hash, cert) VALUES ($1, $2, $3, $4)
     ON CONFLICT DO NOTHING",
  )
  .bind(key_id)
  .bind(role as i16)
  .bind(Sha256::digest(cert_bytes).to_vec())
  .bind(cert_bytes)
  .execute(connection)
  .await?;
  Ok(())
}

pub async fn is_revoked(connection: &mut PgConnection, key_id: &[u8]) -> Result<bool, ApiError> {
  Ok(
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM revoked_server_keys WHERE key_id = $1)")
      .bind(key_id)
      .fetch_one(connection)
      .await?,
  )
}

pub async fn issuance_frozen(connection: &mut PgConnection) -> Result<bool, ApiError> {
  Ok(
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM issuance_freezes)")
      .fetch_one(connection)
      .await?,
  )
}

/// Every key, seal, revocation and freeze check of one stub, on the paying transaction.
/// `None` when the stub may be paid.
pub async fn stub_refusal(
  connection: &mut PgConnection,
  state: &AppState,
  stub: &ChequeStub,
  server_key_id: &[u8],
  cheque_id: &[u8],
) -> Result<Option<KeyRefusal>, ApiError> {
  let issued_at: i64 = sqlx::query_scalar(
    "SELECT floor(extract(epoch FROM b.created_at))::bigint
     FROM cheques c JOIN chequebooks b USING (chequebook_id) WHERE c.cheque_id = $1",
  )
  .bind(cheque_id)
  .fetch_one(&mut *connection)
  .await?;
  let stored: Vec<Vec<u8>> =
    sqlx::query_scalar("SELECT cert FROM server_key_certs WHERE key_id = $1 AND role = $2")
      .bind(server_key_id)
      .bind(ServerKeyRole::Issuer as i16)
      .fetch_all(&mut *connection)
      .await?;
  // A cheque issued under a cert current at issue time stays payable after that cert expires.
  let mut refusal = KeyRefusal::Invalid("unknown server key");
  let mut issuer_key = None;
  for bytes in &stored {
    let cert: ServerKeyCert = decode_stored(bytes, "server key cert")?;
    match verify_cert(&cert, &state.root_public_key, ServerKeyRole::Issuer) {
      Ok(body) if body.key_id != server_key_id => {
        refusal = KeyRefusal::Invalid(CertError::KeyIdMismatch.reason());
      }
      Ok(body) if is_current(&body, to_u64(issued_at)?) => {
        issuer_key = Some(body.pubkey);
        break;
      }
      Ok(_) => refusal = KeyRefusal::Expired,
      Err(error) => refusal = KeyRefusal::Invalid(error.reason()),
    }
  }
  let Some(issuer_key) = issuer_key else {
    return Ok(Some(refusal));
  };
  if verify(
    &issuer_key,
    &stub_seal_message(&stub.body),
    &stub.server_signature,
  )
  .is_err()
  {
    return Ok(Some(KeyRefusal::Invalid("bad server seal")));
  }
  if is_revoked(connection, server_key_id).await? {
    return Ok(Some(KeyRefusal::Revoked));
  }
  let after_freeze: bool = sqlx::query_scalar("SELECT issued_after_freeze($1)")
    .bind(cheque_id)
    .fetch_one(&mut *connection)
    .await?;
  Ok(after_freeze.then_some(KeyRefusal::IssuedAfterFreeze))
}
