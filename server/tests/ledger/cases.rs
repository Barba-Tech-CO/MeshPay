use crate::support::{Client, TestApp, random_bytes, setup, setup_with};
use axum::http::StatusCode;
use meshpay_server::crypto::p256_keys::verify;
use meshpay_server::wire::conflict_kind::ConflictKind;
use meshpay_server::wire::redeem_status::RedeemStatus;

async fn funded(app: &TestApp) -> Client {
  let client = Client::new();
  assert_eq!(client.register(app).await.0, StatusCode::OK);
  client
}

#[tokio::test]
async fn register_grants_once() {
  let Some(app) = setup(true).await else { return };
  let client = Client::new();
  let (_, first) = client.register(&app).await;
  let (_, second) = client.register(&app).await;
  assert!(first.unwrap().created);
  assert!(!second.unwrap().created);
  assert_eq!(app.balance(&client.account_id).await, (10_000, 0));
}

#[tokio::test]
async fn software_accounts_refused_without_lab_mode() {
  let Some(app) = setup(false).await else {
    return;
  };
  assert_eq!(Client::new().register(&app).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn issue_reserves_and_is_idempotent() {
  let Some(app) = setup(true).await else { return };
  let payer = funded(&app).await;
  let request_id = random_bytes(16);
  let (status, book) = payer.issue(&app, request_id.clone(), 3, 100, 60).await;
  let book = book.unwrap();
  assert_eq!((status, book.stubs.len()), (StatusCode::OK, 3));
  assert_eq!(app.balance(&payer.account_id).await, (9_700, 300));
  for stub in &book.stubs {
    let seal = meshpay_server::crypto::signing_messages::stub_seal_message(&stub.body);
    verify(&app.state.public_key(), &seal, &stub.server_signature).unwrap();
  }
  let (_, again) = payer.issue(&app, request_id, 3, 100, 60).await;
  assert_eq!(again.unwrap().chequebook_id, book.chequebook_id);
  assert_eq!(app.balance(&payer.account_id).await, (9_700, 300));
}

#[tokio::test]
async fn over_cap_requests_are_refused() {
  let Some(app) = setup(true).await else { return };
  let payer = funded(&app).await;
  let refused = |count, max, ttl| {
    let payer = &payer;
    let app = &app;
    async move { payer.issue(app, random_bytes(16), count, max, ttl).await.0 }
  };
  assert_eq!(
    refused(1, 1_001, 60).await,
    StatusCode::UNPROCESSABLE_ENTITY,
    "f cap"
  );
  assert_eq!(
    refused(11, 10, 60).await,
    StatusCode::UNPROCESSABLE_ENTITY,
    "N_max"
  );
  assert_eq!(
    refused(10, 1_000, 60).await,
    StatusCode::UNPROCESSABLE_ENTITY,
    "R cap"
  );
  assert_eq!(
    refused(1, 10, 3_601).await,
    StatusCode::UNPROCESSABLE_ENTITY,
    "E cap"
  );
  assert_eq!(refused(5, 1_000, 60).await, StatusCode::OK);
  assert_eq!(
    refused(1, 100, 60).await,
    StatusCode::UNPROCESSABLE_ENTITY,
    "pending cap"
  );
  assert_eq!(app.balance(&payer.account_id).await, (5_000, 5_000));
}

#[tokio::test]
async fn deposit_pays_and_repeat_is_idempotent() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let (_, book) = payer.issue(&app, random_bytes(16), 3, 100, 60).await;
  let stub = &book.unwrap().stubs[0];
  let spend = payer.spend(stub, &payee.account_id, 40, 1);

  let paid = app.deposit(stub, &spend).await;
  assert_eq!(
    (paid.status, paid.credited_units),
    (RedeemStatus::Paid as i32, 40)
  );
  assert_eq!(app.balance(&payee.account_id).await, (10_040, 0));
  assert_eq!(app.balance(&payer.account_id).await, (9_760, 200));

  let again = app.deposit(stub, &spend).await;
  assert_eq!(again.status, RedeemStatus::AlreadyRedeemedSameBody as i32);
  assert_eq!(app.balance(&payee.account_id).await, (10_040, 0));
  assert_eq!(app.balance(&payer.account_id).await, (9_760, 200));
}

#[tokio::test]
async fn bad_signature_and_overspend_are_rejected() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee, thief) = (funded(&app).await, funded(&app).await, funded(&app).await);
  let (_, book) = payer.issue(&app, random_bytes(16), 2, 100, 60).await;
  let stubs = book.unwrap().stubs;
  let forged = thief.spend(&stubs[0], &payee.account_id, 10, 1);
  assert_eq!(
    app.deposit(&stubs[0], &forged).await.status,
    RedeemStatus::RejectedInvalid as i32
  );
  let over = payer.spend(&stubs[1], &payee.account_id, 101, 1);
  assert_eq!(
    app.deposit(&stubs[1], &over).await.status,
    RedeemStatus::RejectedInvalid as i32
  );
  assert_eq!(app.balance(&payee.account_id).await, (10_000, 0));
}

#[tokio::test]
async fn double_spend_same_cheque_gives_proof_and_freezes() {
  let Some(app) = setup(true).await else { return };
  let (payer, first_payee, second_payee) =
    (funded(&app).await, funded(&app).await, funded(&app).await);
  let (_, book) = payer.issue(&app, random_bytes(16), 2, 100, 60).await;
  let stub = &book.unwrap().stubs[0];
  let spend_a = payer.spend(stub, &first_payee.account_id, 40, 1);
  let spend_b = payer.spend(stub, &second_payee.account_id, 50, 1);

  assert_eq!(
    app.deposit(stub, &spend_a).await.status,
    RedeemStatus::Paid as i32
  );
  let conflict = app.deposit(stub, &spend_b).await;
  assert_eq!(conflict.status, RedeemStatus::RejectedConflict as i32);
  let proof = conflict.proof.unwrap();
  assert_eq!(proof.conflict_kind, ConflictKind::SameCheque as i32);
  for spend in [proof.spend_1.unwrap(), proof.spend_2.unwrap()] {
    let message = meshpay_server::crypto::signing_messages::spend_signing_message(&spend.body);
    let stub_body = prost::Message::decode(stub.body.as_slice());
    let stub_body: meshpay_server::wire::cheque_stub_body::ChequeStubBody = stub_body.unwrap();
    verify(&stub_body.signer_pubkey, &message, &spend.payer_signature).unwrap();
  }
  assert_eq!(app.balance(&second_payee.account_id).await, (10_000, 0));
  assert_eq!(app.balance(&first_payee.account_id).await, (10_040, 0));
  let frozen: bool = sqlx::query_scalar("SELECT frozen FROM accounts WHERE account_id = $1")
    .bind(&payer.account_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
  assert!(frozen);
  assert_eq!(
    payer.issue(&app, random_bytes(16), 1, 10, 60).await.0,
    StatusCode::FORBIDDEN
  );
}

#[tokio::test]
async fn same_counter_on_two_cheques_gives_proof_and_freezes() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let (_, book) = payer.issue(&app, random_bytes(16), 2, 100, 60).await;
  let stubs = book.unwrap().stubs;
  let first = payer.spend(&stubs[0], &payee.account_id, 10, 7);
  let second = payer.spend(&stubs[1], &payee.account_id, 20, 7);
  assert_eq!(
    app.deposit(&stubs[0], &first).await.status,
    RedeemStatus::Paid as i32
  );
  let response = app.deposit(&stubs[1], &second).await;
  // The cheque has its own reserve, so it is still paid; the conflict is proven and frozen.
  assert_eq!(response.status, RedeemStatus::Paid as i32);
  assert_eq!(
    response.proof.unwrap().conflict_kind,
    ConflictKind::SameCounter as i32
  );
  let proofs: i64 =
    sqlx::query_scalar("SELECT count(*) FROM equivocation_proofs WHERE offender_account_id = $1")
      .bind(&payer.account_id)
      .fetch_one(&app.pool)
      .await
      .unwrap();
  assert_eq!(proofs, 1);
}

#[tokio::test]
async fn expired_cheque_is_refused_and_reserve_returns() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let (_, book) = payer.issue(&app, random_bytes(16), 2, 100, 1).await;
  let stub = &book.unwrap().stubs[0];
  tokio::time::sleep(std::time::Duration::from_millis(2_100)).await;
  let spend = payer.spend(stub, &payee.account_id, 10, 1);
  assert_eq!(
    app.deposit(stub, &spend).await.status,
    RedeemStatus::RejectedExpired as i32
  );
  assert_eq!(app.balance(&payer.account_id).await, (9_800, 200));
  let (status, _) = payer.issue(&app, random_bytes(16), 1, 50, 60).await;
  assert_eq!(status, StatusCode::OK);
  assert_eq!(app.balance(&payer.account_id).await, (9_950, 50));
}

#[tokio::test]
async fn expired_never_redeemed_deposit_is_refused_without_touching_balances() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let (_, book) = payer.issue(&app, random_bytes(16), 1, 100, 1).await;
  let stub = &book.unwrap().stubs[0];
  tokio::time::sleep(std::time::Duration::from_millis(2_100)).await;
  let spend = payer.spend(stub, &payee.account_id, 10, 1);
  for _ in 0..3 {
    assert_eq!(
      app.deposit(stub, &spend).await.status,
      RedeemStatus::RejectedExpired as i32
    );
  }
  assert_eq!(app.balance(&payee.account_id).await, (10_000, 0));
  assert_eq!(app.balance(&payer.account_id).await, (9_900, 100));
  let redemptions: i64 =
    sqlx::query_scalar("SELECT count(*) FROM redemptions WHERE payer_account_id = $1")
      .bind(&payer.account_id)
      .fetch_one(&app.pool)
      .await
      .unwrap();
  assert_eq!(redemptions, 0);
}

#[tokio::test]
async fn resend_of_paid_body_after_expiry_returns_previous_result() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let (_, book) = payer.issue(&app, random_bytes(16), 1, 100, 1).await;
  let stub = &book.unwrap().stubs[0];
  let spend = payer.spend(stub, &payee.account_id, 10, 1);
  assert_eq!(
    app.deposit(stub, &spend).await.status,
    RedeemStatus::Paid as i32
  );
  tokio::time::sleep(std::time::Duration::from_millis(2_100)).await;
  assert_eq!(
    app.deposit(stub, &spend).await.status,
    RedeemStatus::AlreadyRedeemedSameBody as i32
  );
  assert_eq!(app.balance(&payee.account_id).await, (10_010, 0));
}

#[tokio::test]
async fn conflicting_spend_deposited_after_expiry_gives_proof_and_freezes() {
  let Some(app) = setup(true).await else { return };
  let (payer, first_payee, second_payee) =
    (funded(&app).await, funded(&app).await, funded(&app).await);
  let (_, book) = payer.issue(&app, random_bytes(16), 1, 100, 1).await;
  let stub = &book.unwrap().stubs[0];
  let spend_a = payer.spend(stub, &first_payee.account_id, 40, 1);
  let spend_b = payer.spend(stub, &second_payee.account_id, 50, 1);
  assert_eq!(
    app.deposit(stub, &spend_a).await.status,
    RedeemStatus::Paid as i32
  );
  tokio::time::sleep(std::time::Duration::from_millis(2_100)).await;
  let conflict = app.deposit(stub, &spend_b).await;
  assert_eq!(conflict.status, RedeemStatus::RejectedConflict as i32);
  assert_eq!(
    conflict.proof.unwrap().conflict_kind,
    ConflictKind::SameCheque as i32
  );
  assert_eq!(app.balance(&second_payee.account_id).await, (10_000, 0));
  let frozen: bool = sqlx::query_scalar("SELECT frozen FROM accounts WHERE account_id = $1")
    .bind(&payer.account_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
  assert!(frozen);
}

#[tokio::test]
async fn triggers_stop_even_the_owner_and_invariants_hold() {
  let Some(app) = setup(true).await else { return };
  let payer = funded(&app).await;
  payer.issue(&app, random_bytes(16), 1, 10, 60).await;
  freeze(&app, &payer).await;
  let unfreeze = format!(
    "UPDATE accounts SET frozen = false WHERE account_id = '\\x{}'",
    hex::encode(&payer.account_id)
  );
  for sql in [
    unfreeze.as_str(),
    "UPDATE ledger_entries SET delta_available = delta_available + 1",
    "DELETE FROM ledger_entries",
    "TRUNCATE ledger_entries CASCADE",
    "UPDATE audit_log SET event = 'CHEQUE_RELEASED'",
    "DELETE FROM cheques",
    "UPDATE redemptions SET amount_units = 1",
    "UPDATE supply SET cap = cap + 1",
  ] {
    let error = sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
      .execute(&app.owner_pool)
      .await
      .unwrap_err();
    let text = error.to_string();
    assert!(
      text.contains("append-only") || text.contains("immutable") || text.contains("only be frozen"),
      "{sql}: {text}"
    );
  }
  let overdraw = sqlx::query(
    "INSERT INTO ledger_entries (account_id, kind, delta_available, delta_reserved, ref_id)
     VALUES ($1, 'RESERVE', -100000, 100000, $1)",
  )
  .bind(&payer.account_id)
  .execute(&app.owner_pool)
  .await;
  assert!(overdraw.is_err());
  let over_supply = sqlx::query(
    "INSERT INTO ledger_entries (account_id, kind, delta_available, delta_reserved, ref_id)
     VALUES ($1, 'GRANT', 9000000000000000000, 0, $1)",
  )
  .bind(&payer.account_id)
  .execute(&app.owner_pool)
  .await;
  assert!(over_supply.is_err());
  let supply_ok: bool =
    sqlx::query_scalar("SELECT circulating <= granted AND granted <= cap FROM supply")
      .fetch_one(&app.pool)
      .await
      .unwrap();
  assert!(supply_ok);
}

#[tokio::test]
async fn runtime_role_cannot_bypass_the_ledger() {
  let Some(app) = setup(true).await else { return };
  let payer = funded(&app).await;
  freeze(&app, &payer).await;
  let unfreeze = format!(
    "UPDATE accounts SET frozen = false WHERE account_id = '\\x{}'",
    hex::encode(&payer.account_id)
  );
  for sql in [
    unfreeze.as_str(),
    "ALTER TABLE ledger_entries DISABLE TRIGGER ALL",
    "DROP TRIGGER ledger_entries_apply ON ledger_entries",
    "SET session_replication_role = replica",
    "UPDATE supply SET cap = 1",
    "UPDATE supply SET granted = 0",
    "UPDATE balances SET available = available + 1",
    "INSERT INTO balances (account_id) VALUES ('\\x00')",
    "UPDATE audit_head SET hash = hash",
    "DELETE FROM ledger_entries",
    "UPDATE ledger_entries SET delta_available = 0",
    "TRUNCATE ledger_entries CASCADE",
    "UPDATE accounts SET epoch = 2",
    "INSERT INTO ledger_entries (account_id, kind, delta_available, delta_reserved, ref_id)
     SELECT account_id, 'GRANT', 1000000, 0, account_id FROM accounts LIMIT 1",
    "INSERT INTO accounts (account_id, public_key, attestation_level)
     VALUES (decode(repeat('01', 16), 'hex'), decode(repeat('02', 33), 'hex'), 'lab_software')",
    "INSERT INTO supply (cap, initial_grant) VALUES (1, 1)",
    "SELECT post_reserve(decode(repeat('03', 16), 'hex'))",
    "SELECT post_redeem(decode(repeat('03', 16), 'hex'))",
    "SELECT post_release(decode(repeat('03', 16), 'hex'))",
    "INSERT INTO redemptions (cheque_id, payer_account_id, payee_account_id, epoch, counter,
       amount_units, tx_id, body_hash, spend)
     VALUES (decode(repeat('04', 16), 'hex'), decode(repeat('01', 16), 'hex'),
       decode(repeat('02', 16), 'hex'), 1, 1, 1, decode(repeat('00', 32), 'hex'),
       decode(repeat('00', 32), 'hex'), decode('00', 'hex'))",
    "CREATE TABLE intruder (id int)",
    "CREATE ROLE intruder SUPERUSER",
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

/// Freezing is the one account update the runtime role may do.
async fn freeze(app: &TestApp, client: &Client) {
  sqlx::query("UPDATE accounts SET frozen = true WHERE account_id = $1")
    .bind(&client.account_id)
    .execute(&app.pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn audit_chain_is_valid_and_schema_has_no_json() {
  let Some(app) = setup(true).await else { return };
  funded(&app).await;
  let valid: bool = sqlx::query_scalar("SELECT audit_chain_valid()")
    .fetch_one(&app.pool)
    .await
    .unwrap();
  assert!(valid);
  let json_columns: i64 = sqlx::query_scalar(
    "SELECT count(*) FROM information_schema.columns
     WHERE table_schema = 'public' AND data_type IN ('json', 'jsonb')",
  )
  .fetch_one(&app.pool)
  .await
  .unwrap();
  assert_eq!(json_columns, 0);
}

#[tokio::test]
async fn balance_read_requires_the_accounts_own_fresh_signature() {
  let Some(app) = setup(true).await else { return };
  let (me, other) = (funded(&app).await, funded(&app).await);
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  assert_eq!(
    me.read_balance(&app, &me.account_id, now).await,
    StatusCode::OK
  );
  assert_eq!(
    me.read_balance(&app, &me.account_id, now - 120).await,
    StatusCode::FORBIDDEN
  );
  assert_eq!(
    me.read_balance(&app, &me.account_id, now + 120).await,
    StatusCode::FORBIDDEN
  );
  // My signature over another account's id, and a key that is not registered at all.
  assert_eq!(
    me.read_balance(&app, &other.account_id, now).await,
    StatusCode::FORBIDDEN
  );
  let stranger = Client::new();
  assert_eq!(
    stranger.read_balance(&app, &stranger.account_id, now).await,
    StatusCode::FORBIDDEN
  );
  // Wrong key: signed by `other` but claiming to be `me`.
  let message = [
    b"meshpay/balance/v1".as_slice(),
    &me.account_id,
    &now.to_be_bytes(),
  ]
  .concat();
  let forged = other.sign_raw(&message);
  let path = format!("/v1/accounts/{}/balance", hex::encode(&me.account_id));
  assert_eq!(
    app.get_signed(&path, now, &forged).await.0,
    StatusCode::FORBIDDEN
  );
  // No headers at all.
  assert_eq!(
    app.call("GET", &path, Vec::new()).await.0,
    StatusCode::FORBIDDEN
  );
}

#[tokio::test]
async fn stale_clock_cannot_redeem_a_released_cheque() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let (_, book) = payer.issue(&app, random_bytes(16), 2, 100, 1).await;
  let stub = &book.unwrap().stubs[0];
  tokio::time::sleep(std::time::Duration::from_millis(2_100)).await;
  // A new issue releases the expired cheques and returns their reserve.
  assert_eq!(
    payer.issue(&app, random_bytes(16), 1, 50, 60).await.0,
    StatusCode::OK
  );
  let before = (
    app.balance(&payer.account_id).await,
    app.balance(&payee.account_id).await,
  );
  let request = meshpay_server::wire::deposit_request::DepositRequest {
    stub: Some(stub.clone()),
    spend: Some(payer.spend(stub, &payee.account_id, 10, 1)),
  };
  let response = meshpay_server::ledger::deposit::deposit(&app.state, request).await;
  let response = response.ok().unwrap();
  assert_eq!(response.status, RedeemStatus::RejectedExpired as i32);
  let after = (
    app.balance(&payer.account_id).await,
    app.balance(&payee.account_id).await,
  );
  assert_eq!(before, after);
}

#[tokio::test]
async fn unknown_issuer_gets_the_same_403_as_a_bad_signature() {
  let Some(app) = setup(true).await else { return };
  let stranger = Client::new();
  assert_eq!(
    stranger.issue(&app, random_bytes(16), 1, 10, 60).await.0,
    StatusCode::FORBIDDEN
  );
}

#[tokio::test]
async fn registration_faucet_is_rate_limited() {
  let Some(app) = setup_with(|config| config.register_limit_per_minute = 2).await else {
    return;
  };
  assert_eq!(Client::new().register(&app).await.0, StatusCode::OK);
  assert_eq!(Client::new().register(&app).await.0, StatusCode::OK);
  assert_eq!(
    Client::new().register(&app).await.0,
    StatusCode::TOO_MANY_REQUESTS
  );
}

#[test]
fn config_validation_rejects_out_of_range_values() {
  let valid = || meshpay_server::config::app_config::AppConfig {
    database_url: String::new(),
    listen_addr: String::new(),
    lab_mode: true,
    server_signing_key_hex: String::new(),
    server_root_public_key_hex: String::new(),
    server_key_cert_hex: String::new(),
    server_snapshot_key_hex: String::new(),
    server_snapshot_key_cert_hex: String::new(),
    cap_cheque_units: 1,
    cap_reserve_units: 1,
    cheque_max_count: 10,
    cheque_ttl_max_seconds: 1,
    balance_auth_window_seconds: 1,
    request_timeout_seconds: 1,
    max_concurrent_requests: 1,
    register_limit_per_minute: 1,
    challenge_limit_per_minute: 1,
    control_limit_per_minute: 1,
    public_read_limit_per_minute: 1,
    attestation_roots_pem_file: String::new(),
    attestation_revocation_file: String::new(),
    attestation_challenge_ttl_seconds: 60,
    allow_tee_issuance: false,
  };
  assert!(valid().validate().is_ok());
  let mut config = valid();
  config.attestation_challenge_ttl_seconds = 61;
  assert!(config.validate().is_err());
  let mut config = valid();
  config.cheque_max_count = 11;
  assert!(config.validate().is_err());
  let mut config = valid();
  config.cap_reserve_units = u64::MAX;
  assert!(config.validate().is_err());
  let mut config = valid();
  config.request_timeout_seconds = 0;
  assert!(config.validate().is_err());
}

#[tokio::test]
async fn ledger_entries_must_match_their_source_rows() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (funded(&app).await, funded(&app).await);
  let (_, book) = payer.issue(&app, random_bytes(16), 1, 100, 60).await;
  let book = book.unwrap();
  let stub = &book.stubs[0];
  let spend = payer.spend(stub, &payee.account_id, 40, 1);
  app.deposit(stub, &spend).await;
  let cheque_id: Vec<u8> =
    sqlx::query_scalar("SELECT cheque_id FROM redemptions WHERE payer_account_id = $1")
      .bind(&payer.account_id)
      .fetch_one(&app.pool)
      .await
      .unwrap();
  let insert =
    |kind: &'static str, account: &[u8], available: i64, reserved: i64, reference: &[u8]| {
      let pool = app.owner_pool.clone();
      let (account, reference) = (account.to_vec(), reference.to_vec());
      async move {
        sqlx::query(
          "INSERT INTO ledger_entries (account_id, kind, delta_available, delta_reserved, ref_id)
         VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(account)
        .bind(kind)
        .bind(available)
        .bind(reserved)
        .bind(reference)
        .execute(&pool)
        .await
      }
    };
  // Even the schema owner side cannot post an entry that its source row does not support.
  let fresh = random_bytes(16);
  let (p, q) = (&payer.account_id, &payee.account_id);
  assert!(
    insert("GRANT", q, 10_000, 0, q).await.is_err(),
    "GRANT outside the account trigger"
  );
  assert!(
    insert("RESERVE", p, -5, 5, &fresh).await.is_err(),
    "RESERVE without chequebook"
  );
  assert!(
    insert("CREDIT", q, 41, 0, &cheque_id).await.is_err(),
    "CREDIT with wrong amount"
  );
  assert!(
    insert("CREDIT", p, 40, 0, &cheque_id).await.is_err(),
    "CREDIT to the wrong account"
  );
  assert!(
    insert("RELEASE", p, 100, -100, &cheque_id).await.is_err(),
    "RELEASE of a redeemed cheque"
  );
  assert!(
    insert("SETTLE", p, 60, -100, &fresh).await.is_err(),
    "SETTLE without redemption"
  );
  assert!(
    insert("CREDIT", q, 40, 0, &cheque_id).await.is_err(),
    "CREDIT twice"
  );
  let again = sqlx::query("SELECT post_redeem($1)")
    .bind(&cheque_id)
    .execute(&app.pool)
    .await;
  assert!(again.is_err(), "post_redeem twice");
  assert_eq!(app.balance(q).await, (10_040, 0));
  assert_eq!(app.balance(p).await, (9_960, 0));
}

#[tokio::test]
async fn junk_registrations_do_not_use_the_faucet_budget() {
  let Some(app) = setup_with(|config| config.register_limit_per_minute = 1).await else {
    return;
  };
  let junk = meshpay_server::wire::register_request::RegisterRequest {
    public_key: random_bytes(33),
    signature: random_bytes(64),
    attestation_level: meshpay_server::wire::attestation_level::AttestationLevel::LabSoftware
      as i32,
    attestation_chain: Vec::new(),
    challenge: Vec::new(),
  };
  for _ in 0..3 {
    let body = prost::Message::encode_to_vec(&junk);
    let (status, text) = app.call("POST", "/v1/accounts", body).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(text, b"registration refused");
  }
  assert_eq!(Client::new().register(&app).await.0, StatusCode::OK);
  assert_eq!(
    Client::new().register(&app).await.0,
    StatusCode::TOO_MANY_REQUESTS
  );
}

#[test]
fn db_passwords_must_be_hex() {
  use meshpay_server::config::db_password::{ensure_hex_password, ensure_hex_url_password};
  assert!(ensure_hex_password("p", "0123456789abcdef0123").is_ok());
  assert!(ensure_hex_password("p", "short").is_err());
  assert!(ensure_hex_password("p", "0123456789abcdef01@3").is_err());
  assert!(
    ensure_hex_url_password("u", "postgres://user:0123456789abcdef0123@host:5432/db").is_ok()
  );
  assert!(
    ensure_hex_url_password("u", "postgres://user:pa%ss0123456789abcdef@host:5432/db").is_err()
  );
  assert!(ensure_hex_url_password("u", "postgres://host/db").is_err());
}
