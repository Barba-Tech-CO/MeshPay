use crate::attestation::account_level::AccountLevel;
use crate::crypto::p256_keys::{sign, verify};
use crate::crypto::signing_messages::{issue_message, stub_seal_message};
use crate::error::ApiError;
use crate::ledger::stored::decode_stored;
use crate::ledger::units::{to_i64, to_u64};
use crate::ledger::{audit, balance, expiry};
use crate::state::AppState;
use crate::wire::cheque_stub::ChequeStub;
use crate::wire::cheque_stub_body::ChequeStubBody;
use crate::wire::issue_request::IssueRequest;
use crate::wire::issue_request_body::IssueRequestBody;
use crate::wire::issue_response::IssueResponse;
use prost::Message;

fn random_id() -> Result<Vec<u8>, ApiError> {
  let mut id = vec![0u8; 16];
  getrandom::fill(&mut id).map_err(|e| ApiError::Internal(format!("random: {e}")))?;
  Ok(id)
}

/// Issues a chequebook (talão): checks the caps, reserves `n * f`, seals one stub per cheque.
pub async fn issue(
  state: &AppState,
  request: IssueRequest,
  now_unix: u64,
) -> Result<IssueResponse, ApiError> {
  let body = IssueRequestBody::decode(request.body.as_slice())
    .map_err(|e| ApiError::BadRequest(format!("issue body: {e}")))?;
  if body.account_id.len() != 16 || body.request_id.len() != 16 {
    return Err(ApiError::BadRequest(
      "account_id and request_id are 16 bytes".to_owned(),
    ));
  }
  let config = &state.config;
  if body.cheque_count == 0 || body.cheque_count > config.cheque_max_count.min(10) {
    return Err(ApiError::Unprocessable(
      "cheque_count above N_max".to_owned(),
    ));
  }
  if body.max_amount_units == 0 || body.max_amount_units > config.cap_cheque_units {
    return Err(ApiError::Unprocessable(
      "max_amount_units above cap f".to_owned(),
    ));
  }
  let reserve = body
    .max_amount_units
    .checked_mul(u64::from(body.cheque_count))
    .filter(|r| *r <= config.cap_reserve_units)
    .ok_or(ApiError::Unprocessable("reserve above cap R".to_owned()))?;
  if body.ttl_seconds == 0 || body.ttl_seconds > config.cheque_ttl_max_seconds {
    return Err(ApiError::Unprocessable("ttl_seconds above E".to_owned()));
  }

  // Unknown account and bad signature are indistinguishable to the caller.
  let denied = || ApiError::Forbidden("request not authorized".to_owned());
  let account: Option<(Vec<u8>, i64, AccountLevel)> = sqlx::query_as(
    "SELECT public_key, epoch, attestation_level FROM accounts WHERE account_id = $1",
  )
  .bind(&body.account_id)
  .fetch_optional(&state.pool)
  .await?;
  let (public_key, epoch, level) = account.ok_or_else(denied)?;
  verify(
    &public_key,
    &issue_message(&request.body),
    &request.signature,
  )
  .map_err(|_| denied())?;

  // PRD 4.5, after the signature check so only the account owner learns this.
  if !level.may_issue(config.allow_tee_issuance, config.lab_mode) {
    return Err(ApiError::Forbidden(
      "attestation level cannot issue chequebooks".to_owned(),
    ));
  }
  let mut tx = state.pool.begin().await?;
  let (frozen,): (bool,) =
    sqlx::query_as("SELECT frozen FROM accounts WHERE account_id = $1 FOR UPDATE")
      .bind(&body.account_id)
      .fetch_one(&mut *tx)
      .await?;
  if frozen {
    return Err(ApiError::Forbidden(
      "account frozen by equivocation proof".to_owned(),
    ));
  }
  let existing: Option<(Vec<u8>, i64)> = sqlx::query_as(
    "SELECT chequebook_id, reserve_units FROM chequebooks WHERE account_id = $1 AND request_id = $2",
  )
  .bind(&body.account_id)
  .bind(&body.request_id)
  .fetch_optional(&mut *tx)
  .await?;
  if let Some((chequebook_id, reserve_units)) = existing {
    let stubs: Vec<(Vec<u8>,)> =
      sqlx::query_as("SELECT stub FROM cheques WHERE chequebook_id = $1 ORDER BY cheque_id")
        .bind(&chequebook_id)
        .fetch_all(&mut *tx)
        .await?;
    return Ok(IssueResponse {
      chequebook_id,
      stubs: stubs
        .into_iter()
        .map(|(bytes,)| decode_stored::<ChequeStub>(&bytes, "stub"))
        .collect::<Result<_, _>>()?,
      reserve_units: to_u64(reserve_units)?,
    });
  }

  let now = to_i64(now_unix)?;
  let released = expiry::release_expired(&mut tx, &body.account_id, now).await?;
  let pending = balance::read_pending(&mut tx, &body.account_id).await?;
  if to_u64(pending)? + reserve > config.cap_reserve_units {
    return Err(ApiError::Unprocessable(
      "pending reserve above cap R".to_owned(),
    ));
  }

  let chequebook_id = random_id()?;
  let expires_at_unix = now_unix + body.ttl_seconds;
  let expires_at = to_i64(expires_at_unix)?;
  let units = to_i64(body.max_amount_units)?;
  let reserve_signed = to_i64(reserve)?;
  let epoch_unsigned = to_u64(epoch)?;
  sqlx::query(
    "INSERT INTO chequebooks (chequebook_id, request_id, account_id, epoch, cheque_count,
       max_cheque_units, reserve_units, expires_at_unix) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
  )
  .bind(&chequebook_id)
  .bind(&body.request_id)
  .bind(&body.account_id)
  .bind(epoch)
  .bind(
    i32::try_from(body.cheque_count)
      .map_err(|_| ApiError::Unprocessable("cheque_count".to_owned()))?,
  )
  .bind(units)
  .bind(reserve_signed)
  .bind(expires_at)
  .execute(&mut *tx)
  .await?;
  balance::post_reserve(&mut tx, &chequebook_id).await?;

  let mut stubs = Vec::new();
  for _ in 0..body.cheque_count {
    let stub_body = ChequeStubBody {
      cheque_id: random_id()?,
      chequebook_id: chequebook_id.clone(),
      payer_account_id: body.account_id.clone(),
      payer_epoch: epoch_unsigned,
      signer_pubkey: public_key.clone(),
      max_amount_units: body.max_amount_units,
      expires_at_unix,
      attestation_level: level.wire_level() as i32,
      single_use_key: false,
      server_key_id: state.key_id(),
    };
    let body_bytes = stub_body.encode_to_vec();
    let seal = stub_seal_message(&body_bytes);
    let stub = ChequeStub {
      server_signature: sign(&state.signing_key, &seal),
      body: body_bytes,
    };
    sqlx::query(
      "INSERT INTO cheques (cheque_id, chequebook_id, account_id, epoch, signer_pubkey,
         max_amount_units, expires_at_unix, stub) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(&stub_body.cheque_id)
    .bind(&chequebook_id)
    .bind(&body.account_id)
    .bind(epoch)
    .bind(&public_key)
    .bind(units)
    .bind(expires_at)
    .bind(stub.encode_to_vec())
    .execute(&mut *tx)
    .await?;
    stubs.push(stub);
  }
  audit::append(&mut tx, "CHEQUEBOOK_ISSUED", &chequebook_id).await?;
  for cheque_id in &released {
    audit::append(&mut tx, "CHEQUE_RELEASED", cheque_id).await?;
  }
  tx.commit().await?;
  Ok(IssueResponse {
    chequebook_id,
    stubs,
    reserve_units: reserve,
  })
}
