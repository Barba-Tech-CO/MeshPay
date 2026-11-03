use crate::crypto::p256_keys::{KEY_ID_LEN, verify};
use crate::crypto::signing_messages::spend_signing_message;
use crate::crypto::spend_hashing::{derive_tx_id, spend_hash};
use crate::error::ApiError;
use crate::http::proto_body::unix_now;
use crate::ledger::server_keys::{self, KeyRefusal};
use crate::ledger::stored::decode_stored;
use crate::ledger::units::to_i64;
use crate::ledger::{audit, balance, equivocation};
use crate::state::AppState;
use crate::wire::cheque_spend::ChequeSpend;
use crate::wire::cheque_spend_body::ChequeSpendBody;
use crate::wire::cheque_stub::ChequeStub;
use crate::wire::cheque_stub_body::ChequeStubBody;
use crate::wire::conflict_kind::ConflictKind;
use crate::wire::deposit_request::DepositRequest;
use crate::wire::deposit_response::DepositResponse;
use crate::wire::equivocation_proof::EquivocationProof;
use crate::wire::redeem_status::RedeemStatus;
use prost::Message;

fn outcome(status: RedeemStatus, detail: &str) -> DepositResponse {
  DepositResponse {
    status: status as i32,
    detail: detail.to_owned(),
    proof: None,
    credited_units: 0,
  }
}

fn invalid(detail: &str) -> Result<DepositResponse, ApiError> {
  Ok(outcome(RedeemStatus::RejectedInvalid, detail))
}

impl From<KeyRefusal> for DepositResponse {
  fn from(refusal: KeyRefusal) -> Self {
    match refusal {
      KeyRefusal::Invalid(reason) => outcome(RedeemStatus::RejectedInvalid, reason),
      KeyRefusal::Expired => outcome(
        RedeemStatus::RejectedInvalid,
        "server key cert not current at issue time",
      ),
      KeyRefusal::Revoked => outcome(RedeemStatus::RejectedRevoked, "server key revoked"),
      KeyRefusal::IssuedAfterFreeze => outcome(
        RedeemStatus::RejectedRevoked,
        "stub issued after the issuance freeze",
      ),
    }
  }
}

fn field_len_ok(body: &ChequeSpendBody) -> bool {
  body.cheque_id.len() == 16
    && body.payee_account_id.len() == KEY_ID_LEN
    && body.tx_id.len() == 32
    && (body.prev_spend_hash.is_empty() || body.prev_spend_hash.len() == 32)
}

/// Deposit of one `ChequeSpend` by the receiver (PRD 7.3). Domain outcomes come back as a
/// `DepositResponse` status; only malformed requests and internal failures are `ApiError`.
/// Once the account rows are locked, existing redemptions are consulted first (resend returns the
/// previous result, a different body is an equivocation, both even after expiry); only a cheque
/// never redeemed is then refused by `cheque_releases` or the clock.
pub async fn deposit(
  state: &AppState,
  request: DepositRequest,
) -> Result<DepositResponse, ApiError> {
  let (Some(stub), Some(spend)) = (request.stub, request.spend) else {
    return Err(ApiError::BadRequest(
      "stub and spend are required".to_owned(),
    ));
  };
  let body = ChequeSpendBody::decode(spend.body.as_slice())
    .map_err(|e| ApiError::BadRequest(format!("spend body: {e}")))?;
  if !field_len_ok(&body) {
    return invalid("bad field sizes");
  }
  let (Ok(amount), Ok(counter)) = (to_i64(body.amount_units), to_i64(body.counter)) else {
    return invalid("amount or counter out of range");
  };

  let stored: Option<(Vec<u8>, Vec<u8>, i64, i64)> = sqlx::query_as(
    "SELECT stub, account_id, epoch, max_amount_units FROM cheques WHERE cheque_id = $1",
  )
  .bind(&body.cheque_id)
  .fetch_optional(&state.pool)
  .await?;
  let Some((stub_bytes, payer, epoch, max_amount)) = stored else {
    return invalid("unknown cheque");
  };
  if stub_bytes != stub.encode_to_vec() {
    return invalid("stub differs from the issued stub");
  }
  let stub_body: ChequeStubBody = decode_stored(&stub.body, "stub body")?;
  let message = spend_signing_message(&spend.body);
  if verify(&stub_body.signer_pubkey, &message, &spend.payer_signature).is_err() {
    return invalid("bad payer signature");
  }
  if derive_tx_id(body.counter, &spend.body).ok().as_deref() != Some(body.tx_id.as_slice()) {
    return invalid("tx_id does not match the body");
  }
  if amount == 0 || amount > max_amount {
    return invalid("amount outside 1..=max_amount_units");
  }
  if body.payee_account_id == payer {
    return invalid("payer and payee are the same account");
  }

  // Unlocked early refusal: an expired cheque with no redemption can never become redeemable
  // (a redemption needs an unexpired cheque), so expired spam never takes row locks.
  if unix_now()? > stub_body.expires_at_unix {
    let redeemed: bool =
      sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM redemptions WHERE cheque_id = $1)")
        .bind(&body.cheque_id)
        .fetch_one(&state.pool)
        .await?;
    if !redeemed {
      return Ok(outcome(RedeemStatus::RejectedExpired, "cheque expired"));
    }
  }

  // Both account rows, in id order, so opposite-direction deposits cannot deadlock.
  let mut tx = state.pool.begin().await?;
  let locked: Vec<(Vec<u8>,)> = sqlx::query_as(
    "SELECT account_id FROM accounts WHERE account_id = ANY($1) ORDER BY account_id FOR UPDATE",
  )
  .bind(vec![payer.clone(), body.payee_account_id.clone()])
  .fetch_all(&mut *tx)
  .await?;
  if locked.len() != 2 {
    return invalid("unknown payee");
  }

  let body_hash = spend_hash(&spend.body);
  let paid: Option<(Vec<u8>, Vec<u8>)> =
    sqlx::query_as("SELECT body_hash, spend FROM redemptions WHERE cheque_id = $1")
      .bind(&body.cheque_id)
      .fetch_optional(&mut *tx)
      .await?;
  if let Some((paid_hash, paid_spend)) = paid {
    if paid_hash == body_hash {
      return Ok(outcome(
        RedeemStatus::AlreadyRedeemedSameBody,
        "already paid; same body",
      ));
    }
    let first: ChequeSpend = decode_stored(&paid_spend, "spend")?;
    let (proof, is_new) = equivocation::record_proof(
      &mut tx,
      ConflictKind::SameCheque,
      &payer,
      epoch,
      (&stub, &first),
      (&stub, &spend),
    )
    .await?;
    if is_new {
      audit::append(&mut tx, "EQUIVOCATION_PROVED", &payer).await?;
    }
    tx.commit().await?;
    return Ok(conflict(proof));
  }

  let released: bool =
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM cheque_releases WHERE cheque_id = $1)")
      .bind(&body.cheque_id)
      .fetch_one(&mut *tx)
      .await?;
  if released || unix_now()? > stub_body.expires_at_unix {
    return Ok(outcome(RedeemStatus::RejectedExpired, "cheque expired"));
  }
  // PV-SEC-05, on this transaction: refused after the resend/conflict answers above.
  if let Some(refusal) = server_keys::stub_refusal(
    &mut tx,
    state,
    &stub,
    &stub_body.server_key_id,
    &body.cheque_id,
  )
  .await?
  {
    return Ok(refusal.into());
  }

  let same_counter: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
    "SELECT r.spend, c.stub FROM redemptions r JOIN cheques c USING (cheque_id)
     WHERE r.payer_account_id = $1 AND r.epoch = $2 AND r.counter = $3",
  )
  .bind(&payer)
  .bind(epoch)
  .bind(counter)
  .fetch_optional(&mut *tx)
  .await?;
  let mut proof = None;
  let mut proof_is_new = false;
  if let Some((other_spend, other_stub)) = same_counter {
    let first_spend: ChequeSpend = decode_stored(&other_spend, "spend")?;
    let first_stub: ChequeStub = decode_stored(&other_stub, "stub")?;
    let (found, is_new) = equivocation::record_proof(
      &mut tx,
      ConflictKind::SameCounter,
      &payer,
      epoch,
      (&first_stub, &first_spend),
      (&stub, &spend),
    )
    .await?;
    proof = Some(found);
    proof_is_new = is_new;
  }

  // The first deposit of each cheque_id is always paid: its reserve already covers it.
  sqlx::query(
    "INSERT INTO redemptions (cheque_id, payer_account_id, payee_account_id, epoch, counter,
       amount_units, tx_id, body_hash, spend) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
  )
  .bind(&body.cheque_id)
  .bind(&payer)
  .bind(&body.payee_account_id)
  .bind(epoch)
  .bind(counter)
  .bind(amount)
  .bind(&body.tx_id)
  .bind(&body_hash)
  .bind(spend.encode_to_vec())
  .execute(&mut *tx)
  .await?;
  balance::post_redeem(&mut tx, &body.cheque_id).await?;
  audit::append(&mut tx, "CHEQUE_REDEEMED", &body.cheque_id).await?;
  if proof_is_new {
    audit::append(&mut tx, "EQUIVOCATION_PROVED", &payer).await?;
  }
  tx.commit().await?;

  Ok(DepositResponse {
    status: RedeemStatus::Paid as i32,
    detail: String::new(),
    proof,
    credited_units: body.amount_units,
  })
}

fn conflict(proof: EquivocationProof) -> DepositResponse {
  DepositResponse {
    proof: Some(proof),
    ..outcome(
      RedeemStatus::RejectedConflict,
      "a different body was already paid for this cheque_id",
    )
  }
}
