//! Public key material and the server-signed `RevocationSnapshot` a receiver installs
//! (core `install_snapshot`: raw snapshot bytes, verified with the pinned SNAPSHOT public key).
use crate::support::{app_state_for, setup_isolated, setup_with, unix_now};
use axum::http::StatusCode;
use meshpay_server::crypto::p256_keys::{compressed_public_key, key_id, verify};
use meshpay_server::crypto::server_key_cert::verify_cert;
use meshpay_server::crypto::signing_messages::snapshot_signing_message;
use meshpay_server::wire::revocation_snapshot::RevocationSnapshot;
use meshpay_server::wire::server_key_cert::ServerKeyCert;
use meshpay_server::wire::server_key_response::ServerKeyResponse;
use meshpay_server::wire::server_key_role::ServerKeyRole;
use prost::Message;

#[tokio::test]
async fn server_key_returns_root_issuer_and_snapshot_certs() {
  let Some(app) = setup_isolated(1_000_000, 10_000).await else {
    return;
  };
  let (status, bytes) = app.call("GET", "/v1/server-key", Vec::new()).await;
  assert_eq!(status, StatusCode::OK);
  let response = ServerKeyResponse::decode(bytes.as_slice()).unwrap();
  let root = compressed_public_key(&app.root_key);
  assert_eq!(response.root_public_key, root);
  let issuer = ServerKeyCert::decode(response.issuer_cert.as_slice()).unwrap();
  let issuer_body = verify_cert(&issuer, &root, ServerKeyRole::Issuer).unwrap();
  assert_eq!(issuer_body.pubkey, response.public_key);
  let snapshot = ServerKeyCert::decode(response.snapshot_cert.as_slice()).unwrap();
  verify_cert(&snapshot, &root, ServerKeyRole::Snapshot).unwrap();
  app.drop_isolated().await;
}

#[tokio::test]
async fn revocation_snapshot_is_signed_by_the_snapshot_key_and_lists_revocations() {
  let Some(app) = setup_isolated(1_000_000, 10_000).await else {
    return;
  };
  let revoked = vec![7u8; 16];
  sqlx::query("INSERT INTO revoked_server_keys (key_id, snapshot_hash) VALUES ($1, $2)")
    .bind(&revoked)
    .bind(vec![1u8; 32])
    .execute(&app.owner_pool)
    .await
    .unwrap();
  let before = unix_now();
  let (status, bytes) = app.call("GET", "/v1/revocation-snapshot", Vec::new()).await;
  assert_eq!(status, StatusCode::OK);
  let snapshot = RevocationSnapshot::decode(bytes.as_slice()).unwrap();
  // Same bytes core `snapshot_signing_message` builds: snapshot with the signature cleared.
  let unsigned = RevocationSnapshot {
    server_signature: Vec::new(),
    ..snapshot.clone()
  };
  let snapshot_pubkey = app.state.snapshot_public_key();
  verify(
    &snapshot_pubkey,
    &snapshot_signing_message(&unsigned.encode_to_vec()),
    &snapshot.server_signature,
  )
  .unwrap();
  assert_eq!(snapshot.signer_key_id, key_id(&snapshot_pubkey));
  assert!(snapshot.issued_at_unix >= before && snapshot.issued_at_unix <= unix_now());
  assert!(snapshot.version >= snapshot.issued_at_unix);
  assert_eq!(snapshot.revoked_server_key_ids, vec![revoked]);
  assert!(!snapshot.issuance_frozen);
  app.drop_isolated().await;
}

#[tokio::test]
async fn cached_public_reads_never_429_but_a_snapshot_rebuild_beyond_the_limit_does() {
  let Some(app) = setup_with(|config| config.public_read_limit_per_minute = 1).await else {
    return;
  };
  for _ in 0..5 {
    assert_eq!(
      app.call("GET", "/v1/server-key", Vec::new()).await.0,
      StatusCode::OK
    );
    assert_eq!(
      app
        .call("GET", "/v1/revocation-snapshot", Vec::new())
        .await
        .0,
      StatusCode::OK
    );
  }
  tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
  assert_eq!(
    app
      .call("GET", "/v1/revocation-snapshot", Vec::new())
      .await
      .0,
    StatusCode::TOO_MANY_REQUESTS
  );
}

#[tokio::test]
async fn snapshot_is_cached_briefly_then_rebuilt() {
  let Some(app) = setup_isolated(1_000_000, 10_000).await else {
    return;
  };
  let first = app
    .call("GET", "/v1/revocation-snapshot", Vec::new())
    .await
    .1;
  sqlx::query("INSERT INTO revoked_server_keys (key_id, snapshot_hash) VALUES ($1, $2)")
    .bind(vec![9u8; 16])
    .bind(vec![2u8; 32])
    .execute(&app.owner_pool)
    .await
    .unwrap();
  assert_eq!(
    app
      .call("GET", "/v1/revocation-snapshot", Vec::new())
      .await
      .1,
    first
  );
  tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
  let fresh = RevocationSnapshot::decode(
    app
      .call("GET", "/v1/revocation-snapshot", Vec::new())
      .await
      .1
      .as_slice(),
  )
  .unwrap();
  assert_eq!(fresh.revoked_server_key_ids, vec![vec![9u8; 16]]);
  app.drop_isolated().await;
}

#[tokio::test]
async fn snapshot_version_strictly_increases_past_the_persisted_counter() {
  let Some(app) = setup_isolated(1_000_000, 10_000).await else {
    return;
  };
  let ahead = unix_now() + 1_000;
  sqlx::query("UPDATE snapshot_version SET last_version = $1")
    .bind(i64::try_from(ahead).unwrap())
    .execute(&app.owner_pool)
    .await
    .unwrap();
  let bytes = app
    .call("GET", "/v1/revocation-snapshot", Vec::new())
    .await
    .1;
  let snapshot = RevocationSnapshot::decode(bytes.as_slice()).unwrap();
  assert_eq!(snapshot.version, ahead + 1);
  assert!(snapshot.issued_at_unix < snapshot.version);
  app.drop_isolated().await;
}

#[tokio::test]
async fn startup_refuses_certs_outside_their_window_or_a_shared_key() {
  let now = unix_now();
  let current = (now - 60, now + 3_600);
  let expired = (now - 7_200, now - 3_600);
  let future = (now + 3_600, now + 7_200);
  assert!(app_state_for(current, current, false).is_ok());
  assert!(app_state_for(expired, current, false).is_err());
  assert!(app_state_for(future, current, false).is_err());
  assert!(app_state_for(current, expired, false).is_err());
  assert!(app_state_for(current, future, false).is_err());
  assert!(app_state_for(current, current, true).is_err());
}
