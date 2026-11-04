//! PV-LED-03: reserve release uses a simulated timestamp and is idempotent.
use crate::support::{Client, TestApp, setup, unix_now};
use axum::http::StatusCode;
use meshpay_server::wire::cheque_stub_body::ChequeStubBody;
use meshpay_server::wire::redeem_status::RedeemStatus;
use prost::Message;

async fn release_at(app: &TestApp, account_id: &[u8], now_unix: i64) -> Vec<Vec<u8>> {
  let mut tx = app.pool.begin().await.unwrap();
  let locked: Vec<u8> =
    sqlx::query_scalar("SELECT account_id FROM accounts WHERE account_id = $1 FOR UPDATE")
      .bind(account_id)
      .fetch_one(&mut *tx)
      .await
      .unwrap();
  assert_eq!(locked, account_id);
  let released = meshpay_server::ledger::expiry::release_expired(&mut tx, account_id, now_unix)
    .await
    .unwrap();
  tx.commit().await.unwrap();
  released
}

#[tokio::test]
async fn simulated_expiry_returns_the_full_unspent_reserve_once() {
  let Some(app) = setup(true).await else { return };
  let (payer, payee) = (Client::new(), Client::new());
  assert_eq!(payer.register(&app).await.0, StatusCode::OK);
  assert_eq!(payee.register(&app).await.0, StatusCode::OK);

  let (status, book) = payer.issue(&app, vec![7; 16], 1, 100, 60).await;
  assert_eq!(status, StatusCode::OK);
  let mut book = book.unwrap();
  let chequebook_id = book.chequebook_id.clone();
  let stub = book.stubs.remove(0);
  let body = ChequeStubBody::decode(stub.body.as_slice()).unwrap();
  assert_eq!(app.balance(&payer.account_id).await, (9_900, 100));

  let expires_at = i64::try_from(unix_now()).unwrap() - 300;
  let mut tx = app.owner_pool.begin().await.unwrap();
  sqlx::query("SET LOCAL session_replication_role = 'replica'")
    .execute(&mut *tx)
    .await
    .unwrap();
  sqlx::query("UPDATE chequebooks SET expires_at_unix = $1 WHERE chequebook_id = $2")
    .bind(expires_at)
    .bind(&chequebook_id)
    .execute(&mut *tx)
    .await
    .unwrap();
  sqlx::query("UPDATE cheques SET expires_at_unix = $1 WHERE cheque_id = $2")
    .bind(expires_at)
    .bind(&body.cheque_id)
    .execute(&mut *tx)
    .await
    .unwrap();
  tx.commit().await.unwrap();
  assert!(
    release_at(&app, &payer.account_id, expires_at)
      .await
      .is_empty()
  );
  assert_eq!(app.balance(&payer.account_id).await, (9_900, 100));

  assert_eq!(
    release_at(&app, &payer.account_id, expires_at + 1).await,
    vec![body.cheque_id.clone()]
  );
  assert_eq!(app.balance(&payer.account_id).await, (10_000, 0));
  assert!(
    release_at(&app, &payer.account_id, expires_at + 1)
      .await
      .is_empty()
  );
  assert_eq!(app.balance(&payer.account_id).await, (10_000, 0));

  let spend = payer.spend(&stub, &payee.account_id, 10, 1);
  assert_eq!(
    app.deposit(&stub, &spend).await.status,
    RedeemStatus::RejectedExpired as i32
  );
  assert_eq!(app.balance(&payer.account_id).await, (10_000, 0));
  assert_eq!(app.balance(&payee.account_id).await, (10_000, 0));
}
