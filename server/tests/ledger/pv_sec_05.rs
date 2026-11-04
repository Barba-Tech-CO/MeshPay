//! PV-SEC-05 (server side): ServerKeyCert role/expiry/revocation, issuance kill switch, supply
//! cap enforced by the database and audit_log tamper detection. TLS CA pinning is client side.
use crate::support::{
  Client, TestApp, random_bytes, setup, setup_isolated, setup_with, setup_with_cert, unix_now,
};
use axum::http::StatusCode;
use meshpay_server::crypto::p256_keys::{
  compressed_public_key, key_id, sign, signing_key_from_seed,
};
use meshpay_server::crypto::server_key_cert::issue_cert;
use meshpay_server::crypto::signing_messages::{snapshot_signing_message, stub_seal_message};
use meshpay_server::crypto::spend_hashing::spend_hash;
use meshpay_server::ledger::server_keys;
use meshpay_server::wire::attestation_level::AttestationLevel;
use meshpay_server::wire::cheque_spend_body::ChequeSpendBody;
use meshpay_server::wire::cheque_stub::ChequeStub;
use meshpay_server::wire::cheque_stub_body::ChequeStubBody;
use meshpay_server::wire::kill_switch_request::KillSwitchRequest;
use meshpay_server::wire::redeem_status::RedeemStatus;
use meshpay_server::wire::revocation_snapshot::RevocationSnapshot;
use meshpay_server::wire::server_key_cert::ServerKeyCert;
use meshpay_server::wire::server_key_role::ServerKeyRole;
use p256::ecdsa::SigningKey;
use prost::Message;

fn new_key() -> SigningKey {
  signing_key_from_seed(&random_bytes(32)).unwrap()
}

async fn funded(app: &TestApp) -> Client {
  let client = Client::new();
  assert_eq!(client.register(app).await.0, StatusCode::OK);
  client
}

/// Signed kill-switch body; `signer_cert` is `None` when `signer` is the root.
fn kill_switch(
  signer: &SigningKey,
  signer_cert: Option<ServerKeyCert>,
  issued_at_unix: u64,
  revoked: Vec<Vec<u8>>,
  frozen: bool,
) -> Vec<u8> {
  let mut snapshot = RevocationSnapshot {
    version: unix_now(),
    issued_at_unix,
    revoked_server_key_ids: revoked,
    issuance_frozen: frozen,
    signer_key_id: key_id(&compressed_public_key(signer)),
    ..Default::default()
  };
  snapshot.server_signature = sign(signer, &snapshot_signing_message(&snapshot.encode_to_vec()));
  KillSwitchRequest {
    snapshot: snapshot.encode_to_vec(),
    signer_cert,
  }
  .encode_to_vec()
}

fn sqlstate(error: &sqlx::Error) -> Option<String> {
  error
    .as_database_error()
    .and_then(|e| e.code())
    .map(|c| c.into_owned())
}

/// 23514, the SQLSTATE every PV-SEC-05 trigger and CHECK raises.
const CHECK_VIOLATION: &str = "23514";

/// Writes (as the schema owner) a consistent, reserved chequebook + cheque created at
/// `created_at`, whose stub is sealed by `key` under `cert`, as a compromised or stale issuer
/// would have left it, and returns the stub.
async fn planted_stub(
  app: &TestApp,
  payer: &Client,
  cert: &ServerKeyCert,
  key: &SigningKey,
  created_at: u64,
) -> ChequeStub {
  let key_id_bytes = key_id(&compressed_public_key(key));
  let mut owner = app.owner_pool.acquire().await.unwrap();
  server_keys::register_cert(
    &mut owner,
    &key_id_bytes,
    ServerKeyRole::Issuer,
    &cert.encode_to_vec(),
  )
  .await
  .unwrap();
  let (chequebook_id, expires) = (random_bytes(16), unix_now() + 600);
  sqlx::query(
    "INSERT INTO chequebooks (chequebook_id, request_id, account_id, epoch, cheque_count,
       max_cheque_units, reserve_units, expires_at_unix, created_at)
     VALUES ($1, $2, $3, 1, 1, 10, 10, $4, to_timestamp($5))",
  )
  .bind(&chequebook_id)
  .bind(random_bytes(16))
  .bind(&payer.account_id)
  .bind(expires as i64)
  .bind(created_at as f64)
  .execute(&mut *owner)
  .await
  .unwrap();
  sqlx::query("SELECT post_reserve($1)")
    .bind(&chequebook_id)
    .execute(&mut *owner)
    .await
    .unwrap();
  let body = ChequeStubBody {
    cheque_id: random_bytes(16),
    chequebook_id: chequebook_id.clone(),
    payer_account_id: payer.account_id.clone(),
    payer_epoch: 1,
    signer_pubkey: payer.public_key(),
    max_amount_units: 10,
    expires_at_unix: expires,
    attestation_level: AttestationLevel::LabSoftware as i32,
    single_use_key: false,
    server_key_id: key_id_bytes.clone(),
  };
  let body_bytes = body.encode_to_vec();
  let stub = ChequeStub {
    server_signature: sign(key, &stub_seal_message(&body_bytes)),
    body: body_bytes,
  };
  sqlx::query(
    "INSERT INTO cheques (cheque_id, chequebook_id, account_id, epoch, signer_pubkey,
       max_amount_units, expires_at_unix, stub, server_key_id)
     VALUES ($1, $2, $3, 1, $4, 10, $5, $6, $7)",
  )
  .bind(&body.cheque_id)
  .bind(&chequebook_id)
  .bind(&payer.account_id)
  .bind(payer.public_key())
  .bind(expires as i64)
  .bind(stub.encode_to_vec())
  .bind(&key_id_bytes)
  .execute(&mut *owner)
  .await
  .unwrap();
  stub
}

/// Moves a chequebook's issue time `seconds` into the past (superuser, triggers off).
async fn backdate(app: &TestApp, chequebook_id: &[u8], seconds: f64) {
  let mut tx = app.owner_pool.begin().await.unwrap();
  sqlx::query("SET LOCAL session_replication_role = replica")
    .execute(&mut *tx)
    .await
    .unwrap();
  sqlx::query(
    "UPDATE chequebooks SET created_at = created_at - make_interval(secs => $2)
     WHERE chequebook_id = $1",
  )
  .bind(chequebook_id)
  .bind(seconds)
  .execute(&mut *tx)
  .await
  .unwrap();
  tx.commit().await.unwrap();
}

#[tokio::test]
async fn fifty_stubs_under_bad_server_keys_are_refused() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let now = unix_now();
  let other_root = new_key();
  let mut cases: Vec<(SigningKey, ServerKeyCert, bool)> = Vec::new();
  let mut add = |root: &SigningKey, role, not_before, not_after, revoke| {
    let key = new_key();
    let cert = issue_cert(root, &key, role, not_before, not_after);
    cases.push((key, cert, revoke));
  };
  for k in 1..=15 {
    add(
      &app.root_key,
      ServerKeyRole::Issuer,
      now - 3_600 - k * 100,
      now - k * 100,
      false,
    );
  }
  for k in 1..=5 {
    add(
      &app.root_key,
      ServerKeyRole::Issuer,
      now + k * 100,
      now + 3_600,
      false,
    );
  }
  for role in [
    ServerKeyRole::Snapshot,
    ServerKeyRole::Tls,
    ServerKeyRole::Unspecified,
  ] {
    for _ in 0..5 {
      add(&app.root_key, role, now - 60, now + 3_600, false);
    }
  }
  for _ in 0..10 {
    add(
      &app.root_key,
      ServerKeyRole::Issuer,
      now - 60,
      now + 3_600,
      true,
    );
  }
  for _ in 0..3 {
    add(
      &other_root,
      ServerKeyRole::Issuer,
      now - 60,
      now + 3_600,
      false,
    );
  }
  for _ in 0..2 {
    add(
      &app.root_key,
      ServerKeyRole::Issuer,
      now - 60,
      now + 31 * 86_400,
      false,
    );
  }
  assert_eq!(cases.len(), 50);

  let revoked = cases
    .iter()
    .filter(|(_, _, revoke)| *revoke)
    .map(|(key, _, _)| key_id(&compressed_public_key(key)))
    .collect();
  let body = kill_switch(&app.root_key, None, now, revoked, false);
  assert_eq!(
    app.call("POST", "/v1/control/snapshot", body).await.0,
    StatusCode::NO_CONTENT
  );

  let before = app.balance(&payee.account_id).await;
  for (counter, (key, cert, revoke)) in cases.iter().enumerate() {
    let stub = planted_stub(&app, &payer, cert, key, now).await;
    let spend = payer.spend(&stub, &payee.account_id, 5, counter as u64);
    let status = app.deposit(&stub, &spend).await.status;
    let expected = if *revoke {
      RedeemStatus::RejectedRevoked
    } else {
      RedeemStatus::RejectedInvalid
    };
    assert_eq!(status, expected as i32, "case {counter}");
  }
  assert_eq!(app.balance(&payee.account_id).await, before);
}

#[tokio::test]
async fn revoking_the_live_issuer_key_by_snapshot_key_stops_deposit_and_issuance() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let (_, issued) = payer.issue(&app, random_bytes(16), 2, 10, 600).await;
  let stubs = issued.unwrap().stubs;
  let paid = app
    .deposit(&stubs[0], &payer.spend(&stubs[0], &payee.account_id, 5, 0))
    .await;
  assert_eq!(paid.status, RedeemStatus::Paid as i32);

  let now = unix_now();
  let snapshot_key = new_key();
  let snapshot_cert = issue_cert(
    &app.root_key,
    &snapshot_key,
    ServerKeyRole::Snapshot,
    now - 60,
    now + 3_600,
  );
  // Refused signers: an ISSUER-role cert, an expired SNAPSHOT cert, no cert, another root.
  let issuer_cert = issue_cert(
    &app.root_key,
    &snapshot_key,
    ServerKeyRole::Issuer,
    now - 60,
    now + 60,
  );
  let expired_cert = issue_cert(
    &app.root_key,
    &snapshot_key,
    ServerKeyRole::Snapshot,
    now - 7_200,
    now - 3_600,
  );
  let target = vec![app.state.key_id()];
  for body in [
    kill_switch(&snapshot_key, Some(issuer_cert), now, target.clone(), false),
    kill_switch(
      &snapshot_key,
      Some(expired_cert),
      now,
      target.clone(),
      false,
    ),
    kill_switch(&snapshot_key, None, now, target.clone(), false),
    kill_switch(&new_key(), None, now, target.clone(), false),
  ] {
    assert_eq!(
      app.call("POST", "/v1/control/snapshot", body).await.0,
      StatusCode::FORBIDDEN
    );
  }
  let resend = payer.spend(&stubs[0], &payee.account_id, 5, 0);
  assert_eq!(
    app.deposit(&stubs[0], &resend).await.status,
    RedeemStatus::AlreadyRedeemedSameBody as i32
  );

  let body = kill_switch(&snapshot_key, Some(snapshot_cert), now, target, false);
  assert_eq!(
    app.call("POST", "/v1/control/snapshot", body).await.0,
    StatusCode::NO_CONTENT
  );
  let refused = app
    .deposit(&stubs[1], &payer.spend(&stubs[1], &payee.account_id, 5, 1))
    .await;
  assert_eq!(refused.status, RedeemStatus::RejectedRevoked as i32);
  assert_eq!(
    payer.issue(&app, random_bytes(16), 1, 10, 600).await.0,
    StatusCode::FORBIDDEN
  );
}

#[tokio::test]
async fn kill_switch_refuses_issuance_and_post_freeze_stubs() {
  let Some(app) = setup_isolated(1_000_000, 10_000).await else {
    return;
  };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  // Issued 120 s before a freeze point dated 30 s ago, then 10 issued after it, before the
  // server learned of the freeze.
  let (_, before) = payer.issue(&app, random_bytes(16), 1, 10, 600).await;
  let before = before.unwrap();
  backdate(&app, &before.chequebook_id, 120.0).await;
  let before = before.stubs;
  let freeze_point = unix_now() - 30;
  let (_, after) = payer.issue(&app, random_bytes(16), 10, 10, 600).await;
  let after = after.unwrap().stubs;

  let body = kill_switch(&app.root_key, None, freeze_point, Vec::new(), true);
  assert_eq!(
    app
      .call("POST", "/v1/control/snapshot", body.clone())
      .await
      .0,
    StatusCode::NO_CONTENT
  );
  // Replay changes nothing.
  assert_eq!(
    app.call("POST", "/v1/control/snapshot", body).await.0,
    StatusCode::NO_CONTENT
  );

  for _ in 0..10 {
    let status = payer.issue(&app, random_bytes(16), 1, 10, 600).await.0;
    assert_eq!(status, StatusCode::FORBIDDEN);
  }
  for (counter, stub) in after.iter().enumerate() {
    let spend = payer.spend(stub, &payee.account_id, 5, counter as u64);
    assert_eq!(
      app.deposit(stub, &spend).await.status,
      RedeemStatus::RejectedRevoked as i32
    );
  }
  // A stub issued before the freeze point is still honoured.
  let spend = payer.spend(&before[0], &payee.account_id, 5, 100);
  assert_eq!(
    app.deposit(&before[0], &spend).await.status,
    RedeemStatus::Paid as i32
  );
  // Database backstop: the runtime role cannot write a chequebook while frozen.
  let direct = sqlx::query(
    "INSERT INTO chequebooks (chequebook_id, request_id, account_id, epoch, cheque_count,
       max_cheque_units, reserve_units, expires_at_unix) VALUES ($1, $2, $3, 1, 1, 10, 10, 1)",
  )
  .bind(random_bytes(16))
  .bind(random_bytes(16))
  .bind(&payer.account_id)
  .execute(&app.pool)
  .await
  .unwrap_err();
  assert_eq!(
    sqlstate(&direct).as_deref(),
    Some(CHECK_VIOLATION),
    "{direct}"
  );
  for sql in ["DELETE FROM issuance_freezes", "TRUNCATE issuance_freezes"] {
    assert!(
      sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(&app.pool)
        .await
        .is_err(),
      "{sql}"
    );
  }
  app.drop_isolated().await;
}

#[tokio::test]
async fn issuance_above_the_supply_cap_is_refused_by_the_database_20_of_20() {
  let Some(app) = setup_isolated(50_000, 10_000).await else {
    return;
  };
  for _ in 0..5 {
    funded(&app).await;
  }
  for _ in 0..20 {
    let (status, _) = Client::new().register(&app).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
  }
  let (granted, cap, accounts): (i64, i64, i64) =
    sqlx::query_as("SELECT granted, cap, (SELECT count(*) FROM accounts) FROM supply")
      .fetch_one(&app.pool)
      .await
      .unwrap();
  assert_eq!((granted, cap, accounts), (50_000, 50_000, 5));
  // The cap is not configurable by the runtime role.
  for sql in [
    "UPDATE supply SET cap = cap + 1",
    "INSERT INTO supply (cap, initial_grant) VALUES (1, 1)",
  ] {
    assert!(
      sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(&app.pool)
        .await
        .is_err()
    );
  }
  app.drop_isolated().await;
}

#[tokio::test]
async fn runtime_role_cannot_change_audit_log() {
  let Some(app) = setup(true).await else { return };
  funded(&app).await;
  for sql in [
    "UPDATE audit_log SET ref_id = ref_id",
    "DELETE FROM audit_log",
    "TRUNCATE audit_log",
    "UPDATE audit_head SET hash = hash",
  ] {
    assert!(
      sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(&app.pool)
        .await
        .is_err(),
      "runtime role ran: {sql}"
    );
  }
}

#[tokio::test]
async fn tampered_audit_log_fails_chain_verification() {
  // Superuser with triggers disabled: the only way left to touch the log.
  for tamper in [
    "UPDATE audit_log SET ref_id = '\\x00' WHERE seq = (SELECT min(seq) FROM audit_log)",
    "UPDATE audit_log SET event = 'CHEQUE_RELEASED' WHERE seq = (SELECT min(seq) + 1 FROM audit_log)",
    "DELETE FROM audit_log WHERE seq = (SELECT min(seq) + 1 FROM audit_log)",
    "DELETE FROM audit_log WHERE seq = (SELECT min(seq) FROM audit_log)",
    "DELETE FROM audit_log WHERE seq = (SELECT max(seq) FROM audit_log)",
  ] {
    let Some(app) = setup_isolated(1_000_000, 10_000).await else {
      return;
    };
    for _ in 0..3 {
      funded(&app).await;
    }
    let valid = || async {
      sqlx::query_scalar::<_, bool>("SELECT audit_chain_valid()")
        .fetch_one(&app.pool)
        .await
        .unwrap()
    };
    assert!(valid().await);
    let mut tx = app.owner_pool.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = replica")
      .execute(&mut *tx)
      .await
      .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(tamper.to_owned()))
      .execute(&mut *tx)
      .await
      .unwrap();
    tx.commit().await.unwrap();
    assert!(!valid().await, "undetected: {tamper}");
    app.drop_isolated().await;
  }
}

async fn freeze_counts(app: &TestApp) -> (i64, i64) {
  sqlx::query_as(
    "SELECT (SELECT count(*) FROM issuance_freezes),
            (SELECT count(*) FROM audit_log WHERE event = 'ISSUANCE_FROZEN')",
  )
  .fetch_one(&app.pool)
  .await
  .unwrap()
}

#[tokio::test]
async fn backdated_freeze_only_reaches_the_skew_window() {
  let Some(app) = setup_isolated(1_000_000, 10_000).await else {
    return;
  };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let (_, old) = payer.issue(&app, random_bytes(16), 1, 10, 600).await;
  let old = old.unwrap();
  backdate(&app, &old.chequebook_id, 120.0).await;
  let (_, new) = payer.issue(&app, random_bytes(16), 1, 10, 600).await;
  let new = new.unwrap().stubs;

  let body = kill_switch(&app.root_key, None, 0, Vec::new(), true);
  assert_eq!(
    app.call("POST", "/v1/control/snapshot", body).await.0,
    StatusCode::NO_CONTENT
  );
  let frozen_at: i64 = sqlx::query_scalar("SELECT frozen_at_unix FROM issuance_freezes")
    .fetch_one(&app.pool)
    .await
    .unwrap();
  assert!(frozen_at >= (unix_now() - 61) as i64, "{frozen_at}");
  let spend = payer.spend(&old.stubs[0], &payee.account_id, 5, 0);
  assert_eq!(
    app.deposit(&old.stubs[0], &spend).await.status,
    RedeemStatus::Paid as i32
  );
  let spend = payer.spend(&new[0], &payee.account_id, 5, 1);
  assert_eq!(
    app.deposit(&new[0], &spend).await.status,
    RedeemStatus::RejectedRevoked as i32
  );
  // The runtime role cannot backdate a freeze by inserting it directly either, nor by passing
  // an old created_at (the trigger stamps now(), so the CHECK still sees the real insert time).
  let direct = sqlx::query(
    "INSERT INTO issuance_freezes (snapshot_hash, frozen_at_unix, created_at)
     VALUES ($1, 0, to_timestamp(0))",
  )
  .bind(random_bytes(32))
  .execute(&app.pool)
  .await
  .unwrap_err();
  assert_eq!(
    sqlstate(&direct).as_deref(),
    Some(CHECK_VIOLATION),
    "{direct}"
  );
  app.drop_isolated().await;
}

#[tokio::test]
async fn runtime_role_cannot_set_created_at_or_omit_the_seal_key() {
  let Some(app) = setup_isolated(1_000_000, 10_000).await else {
    return;
  };
  let payer = funded(&app).await;
  let chequebook_id = random_bytes(16);
  let expires = (unix_now() + 600) as i64;
  sqlx::query(
    "INSERT INTO chequebooks (chequebook_id, request_id, account_id, epoch, cheque_count,
       max_cheque_units, reserve_units, expires_at_unix, created_at)
     VALUES ($1, $2, $3, 1, 1, 10, 10, $4, to_timestamp(0))",
  )
  .bind(&chequebook_id)
  .bind(random_bytes(16))
  .bind(&payer.account_id)
  .bind(expires)
  .execute(&app.pool)
  .await
  .unwrap();
  let age: f64 = sqlx::query_scalar(
    "SELECT extract(epoch FROM now() - created_at)::float8 FROM chequebooks WHERE chequebook_id = $1",
  )
  .bind(&chequebook_id)
  .fetch_one(&app.pool)
  .await
  .unwrap();
  assert!(age < 60.0, "{age}");
  let null_key = sqlx::query(
    "INSERT INTO cheques (cheque_id, chequebook_id, account_id, epoch, signer_pubkey,
       max_amount_units, expires_at_unix, stub)
     VALUES ($1, $2, $3, 1, $4, 10, $5, $6)",
  )
  .bind(random_bytes(16))
  .bind(&chequebook_id)
  .bind(&payer.account_id)
  .bind(payer.public_key())
  .bind(expires)
  .bind(random_bytes(8))
  .execute(&app.pool)
  .await
  .unwrap_err();
  assert_eq!(
    sqlstate(&null_key).as_deref(),
    Some(CHECK_VIOLATION),
    "{null_key}"
  );
  app.drop_isolated().await;
}

#[tokio::test]
async fn snapshot_replays_add_no_rows_and_padding_is_refused() {
  let Some(app) = setup_isolated(1_000_000, 10_000).await else {
    return;
  };
  let now = unix_now();
  let body = kill_switch(&app.root_key, None, now - 30, Vec::new(), true);
  let post = |body: Vec<u8>| app.call("POST", "/v1/control/snapshot", body);
  assert_eq!(post(body.clone()).await.0, StatusCode::NO_CONTENT);
  assert_eq!(freeze_counts(&app).await, (1, 1));
  // Same bytes, and same content re-signed with another issue time: no new rows.
  assert_eq!(post(body.clone()).await.0, StatusCode::NO_CONTENT);
  let resigned = kill_switch(&app.root_key, None, now - 20, Vec::new(), true);
  assert_eq!(post(resigned).await.0, StatusCode::NO_CONTENT);
  assert_eq!(freeze_counts(&app).await, (1, 1));
  // Unknown field 99 appended: still verifies, but not the canonical encoding.
  let mut request = KillSwitchRequest::decode(body.as_slice()).unwrap();
  request.snapshot.extend_from_slice(&[0x98, 0x06, 0x00]);
  assert_eq!(
    post(request.encode_to_vec()).await.0,
    StatusCode::BAD_REQUEST
  );
  assert_eq!(freeze_counts(&app).await, (1, 1));
  app.drop_isolated().await;
}

#[tokio::test]
async fn cheque_under_a_cert_current_at_issue_stays_payable_after_expiry() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let now = unix_now();
  let key = new_key();
  let expired = issue_cert(
    &app.root_key,
    &key,
    ServerKeyRole::Issuer,
    now - 3_600,
    now - 1_800,
  );
  let stub = planted_stub(&app, &payer, &expired, &key, now - 2_700).await;
  let spend = payer.spend(&stub, &payee.account_id, 5, 0);
  assert_eq!(
    app.deposit(&stub, &spend).await.status,
    RedeemStatus::Paid as i32
  );
  // Issued after that cert expired: refused.
  let late = planted_stub(&app, &payer, &expired, &key, now).await;
  let spend = payer.spend(&late, &payee.account_id, 5, 1);
  assert_eq!(
    app.deposit(&late, &spend).await.status,
    RedeemStatus::RejectedInvalid as i32
  );
}

#[tokio::test]
async fn renewed_cert_is_used_and_one_key_holds_both_roles() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let now = unix_now();
  let key = new_key();
  let old = issue_cert(
    &app.root_key,
    &key,
    ServerKeyRole::Issuer,
    now - 7_200,
    now - 3_600,
  );
  let renewed = issue_cert(
    &app.root_key,
    &key,
    ServerKeyRole::Issuer,
    now - 60,
    now + 3_600,
  );
  planted_stub(&app, &payer, &old, &key, now - 5_000).await;
  let stub = planted_stub(&app, &payer, &renewed, &key, now).await;
  let spend = payer.spend(&stub, &payee.account_id, 5, 0);
  assert_eq!(
    app.deposit(&stub, &spend).await.status,
    RedeemStatus::Paid as i32
  );
  // The same key as a SNAPSHOT signer: its cert is stored next to the ISSUER ones.
  let snapshot_cert = issue_cert(
    &app.root_key,
    &key,
    ServerKeyRole::Snapshot,
    now - 60,
    now + 3_600,
  );
  let body = kill_switch(&key, Some(snapshot_cert), now, Vec::new(), false);
  assert_eq!(
    app.call("POST", "/v1/control/snapshot", body).await.0,
    StatusCode::NO_CONTENT
  );
  let rows: Vec<(i16,)> = sqlx::query_as("SELECT role FROM server_key_certs WHERE key_id = $1")
    .bind(key_id(&compressed_public_key(&key)))
    .fetch_all(&app.pool)
    .await
    .unwrap();
  let roles: Vec<i16> = rows.into_iter().map(|(role,)| role).collect();
  assert_eq!(roles.len(), 3);
  assert!(roles.contains(&(ServerKeyRole::Snapshot as i16)));
}

#[tokio::test]
async fn issuance_needs_a_current_issuer_cert_covering_the_cheque_lifetime() {
  let now = unix_now();
  let Some(short) = setup_with_cert(now - 60, now + 300).await else {
    return;
  };
  let payer = funded(&short).await;
  assert_eq!(
    payer.issue(&short, random_bytes(16), 1, 10, 600).await.0,
    StatusCode::UNPROCESSABLE_ENTITY
  );
  assert_eq!(
    payer.issue(&short, random_bytes(16), 1, 10, 100).await.0,
    StatusCode::OK
  );
}

#[tokio::test]
async fn database_refuses_redeeming_a_stub_of_a_revoked_key() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let (_, issued) = payer.issue(&app, random_bytes(16), 1, 10, 600).await;
  let stub = issued.unwrap().stubs.remove(0);
  let body = kill_switch(
    &app.root_key,
    None,
    unix_now(),
    vec![app.state.key_id()],
    false,
  );
  assert_eq!(
    app.call("POST", "/v1/control/snapshot", body).await.0,
    StatusCode::NO_CONTENT
  );
  let spend = payer.spend(&stub, &payee.account_id, 5, 0);
  let spend_body = ChequeSpendBody::decode(spend.body.as_slice()).unwrap();
  // The runtime role skipping the server checks still hits the trigger.
  let direct = sqlx::query(
    "INSERT INTO redemptions (cheque_id, payer_account_id, payee_account_id, epoch, counter,
       amount_units, tx_id, body_hash, spend) VALUES ($1, $2, $3, 1, 0, 5, $4, $5, $6)",
  )
  .bind(&spend_body.cheque_id)
  .bind(&payer.account_id)
  .bind(&payee.account_id)
  .bind(&spend_body.tx_id)
  .bind(spend_hash(&spend.body))
  .bind(spend.encode_to_vec())
  .execute(&app.pool)
  .await
  .unwrap_err();
  assert_eq!(
    sqlstate(&direct).as_deref(),
    Some(CHECK_VIOLATION),
    "{direct}"
  );
}

#[tokio::test]
async fn control_snapshot_is_rate_limited() {
  let Some(app) = setup_with(|config| config.control_limit_per_minute = 2).await else {
    return;
  };
  for expected in [
    StatusCode::BAD_REQUEST,
    StatusCode::BAD_REQUEST,
    StatusCode::TOO_MANY_REQUESTS,
  ] {
    let status = app.call("POST", "/v1/control/snapshot", vec![0xff]).await.0;
    assert_eq!(status, expected);
  }
}

#[tokio::test]
async fn root_signed_snapshot_passes_an_exhausted_control_limit() {
  let Some(app) = setup_with(|config| config.control_limit_per_minute = 1).await else {
    return;
  };
  for _ in 0..2 {
    app.call("POST", "/v1/control/snapshot", vec![0xff]).await;
  }
  let body = kill_switch(&app.root_key, None, unix_now(), Vec::new(), false);
  assert_eq!(
    app.call("POST", "/v1/control/snapshot", body).await.0,
    StatusCode::NO_CONTENT
  );
}

#[tokio::test]
async fn runtime_role_does_not_own_the_kill_switch_tables() {
  let Some(app) = setup(true).await else { return };
  let tables = [
    "server_key_certs",
    "revoked_server_keys",
    "issuance_freezes",
  ];
  for table in tables {
    let owner: String = sqlx::query_scalar("SELECT tableowner FROM pg_tables WHERE tablename = $1")
      .bind(table)
      .fetch_one(&app.pool)
      .await
      .unwrap();
    assert_eq!(owner, "meshpay_owner", "{table}");
    let sql = format!("ALTER TABLE {table} DISABLE TRIGGER ALL");
    assert!(
      sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(&app.pool)
        .await
        .is_err(),
      "{table}"
    );
  }
  let member: bool =
    sqlx::query_scalar("SELECT pg_has_role('meshpay_app', 'meshpay_owner', 'MEMBER')")
      .fetch_one(&app.pool)
      .await
      .unwrap();
  assert!(!member);
}
