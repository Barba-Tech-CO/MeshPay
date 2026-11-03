//! Registration with Android Key Attestation against the compose Postgres.
use crate::attestation_pki::{KeySpec, Pki};
use crate::support::{Client, TestApp, random_bytes, setup_with};
use axum::http::StatusCode;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use meshpay_server::config::app_config::AppConfig;
use meshpay_server::wire::challenge_response::ChallengeResponse;
use prost::Message;

const REFUSED: &[u8] = b"registration refused";

async fn attest_app(tweak: impl FnOnce(&mut AppConfig)) -> Option<(TestApp, Pki)> {
  let pki = Pki::new();
  let dir = std::env::temp_dir().join(format!("meshpay-att-{}", hex::encode(random_bytes(8))));
  std::fs::create_dir_all(&dir).unwrap();
  let body = STANDARD.encode(&pki.root_der);
  let lines: Vec<&str> = body
    .as_bytes()
    .chunks(64)
    .map(|c| std::str::from_utf8(c).unwrap())
    .collect();
  let pem = format!(
    "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
    lines.join("\n")
  );
  let roots = dir.join("roots.pem");
  let revoked = dir.join("revoked.txt");
  std::fs::write(&roots, pem).unwrap();
  std::fs::write(&revoked, "").unwrap();
  let app = setup_with(|config| {
    config.lab_mode = false;
    config.attestation_roots_pem_file = roots.to_string_lossy().into_owned();
    config.attestation_revocation_file = revoked.to_string_lossy().into_owned();
    tweak(config);
  })
  .await?;
  Some((app, pki))
}

async fn nonce(app: &TestApp) -> Vec<u8> {
  let (status, bytes) = app.call("GET", "/v1/challenge", Vec::new()).await;
  assert_eq!(status, StatusCode::OK);
  let challenge = ChallengeResponse::decode(bytes.as_slice()).unwrap();
  assert_eq!(challenge.nonce.len(), 32);
  challenge.nonce
}

async fn register(
  app: &TestApp,
  client: &Client,
  chain: Vec<Vec<u8>>,
  challenge: Vec<u8>,
) -> (StatusCode, Vec<u8>) {
  let request = client.attested_request(chain, challenge);
  app
    .call("POST", "/v1/accounts", request.encode_to_vec())
    .await
}

async fn level(app: &TestApp, client: &Client) -> Option<String> {
  sqlx::query_scalar("SELECT attestation_level::text FROM accounts WHERE account_id = $1")
    .bind(&client.account_id)
    .fetch_optional(&app.pool)
    .await
    .unwrap()
}

async fn register_ok(app: &TestApp, pki: &Pki, spec: impl Fn(&[u8]) -> KeySpec) -> Client {
  let client = Client::new();
  let n = nonce(app).await;
  let chain = pki.valid_chain(&client.verifying_key(), &spec(&n));
  assert_eq!(register(app, &client, chain, n).await.0, StatusCode::OK);
  client
}

#[tokio::test]
async fn strongbox_registers_and_issues() {
  let Some((app, pki)) = attest_app(|_| {}).await else {
    return;
  };
  let client = register_ok(&app, &pki, KeySpec::strongbox).await;
  assert_eq!(level(&app, &client).await.as_deref(), Some("strongbox"));
  assert_eq!(app.balance(&client.account_id).await, (10_000, 0));
  let (status, book) = client.issue(&app, random_bytes(16), 2, 100, 60).await;
  assert_eq!(status, StatusCode::OK);
  let stub = book.unwrap().stubs[0].body.clone();
  let body =
    meshpay_server::wire::cheque_stub_body::ChequeStubBody::decode(stub.as_slice()).unwrap();
  assert_eq!(
    body.attestation_level,
    meshpay_server::wire::attestation_level::AttestationLevel::AndroidStrongbox as i32
  );
}

#[tokio::test]
async fn tee_receives_only_unless_the_operator_allows_issuing() {
  let Some((app, pki)) = attest_app(|_| {}).await else {
    return;
  };
  let client = register_ok(&app, &pki, KeySpec::tee).await;
  assert_eq!(level(&app, &client).await.as_deref(), Some("tee"));
  let (status, _) = client.issue(&app, random_bytes(16), 1, 100, 60).await;
  assert_eq!(status, StatusCode::FORBIDDEN);
  assert_eq!(app.balance(&client.account_id).await, (10_000, 0));

  let Some((app, pki)) = attest_app(|config| config.allow_tee_issuance = true).await else {
    return;
  };
  let client = register_ok(&app, &pki, KeySpec::tee).await;
  let (status, _) = client.issue(&app, random_bytes(16), 1, 100, 60).await;
  assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn software_attested_never_issues() {
  let Some((app, pki)) = attest_app(|config| {
    config.lab_mode = true;
    config.allow_tee_issuance = true;
  })
  .await
  else {
    return;
  };
  let client = register_ok(&app, &pki, KeySpec::software).await;
  assert_eq!(
    level(&app, &client).await.as_deref(),
    Some("software_attested")
  );
  let (status, _) = client.issue(&app, random_bytes(16), 1, 100, 60).await;
  assert_eq!(status, StatusCode::FORBIDDEN);
}

/// One refusal scenario: a fresh client and nonce, with the chain built from them.
type Case = (&'static str, Client, Vec<Vec<u8>>, Vec<u8>);

async fn case(
  app: &TestApp,
  name: &'static str,
  build: impl FnOnce(&Client, &[u8]) -> Vec<Vec<u8>>,
) -> Case {
  let client = Client::new();
  let n = nonce(app).await;
  let chain = build(&client, &n);
  (name, client, chain, n)
}

#[tokio::test]
async fn every_refusal_is_the_same_403_and_creates_nothing() {
  let Some((app, pki)) = attest_app(|_| {}).await else {
    return;
  };
  let rogue = Pki::new();
  let strongbox = |client: &Client, pki: &Pki, n: &[u8]| {
    pki.valid_chain(&client.verifying_key(), &KeySpec::strongbox(n))
  };
  let mut refused = vec![
    case(&app, "wrong challenge", |c, _| {
      strongbox(c, &pki, &random_bytes(32))
    })
    .await,
    case(&app, "key mismatch", |_, n| {
      strongbox(&Client::new(), &pki, n)
    })
    .await,
    case(&app, "untrusted root", |c, n| strongbox(c, &rogue, n)).await,
    case(&app, "missing extension", |c, _| {
      pki.chain(
        &c.verifying_key(),
        None,
        9,
        rcgen::date_time_ymd(2098, 1, 1),
      )
    })
    .await,
    case(&app, "broken signature", |c, n| {
      let mut chain = strongbox(c, &pki, n);
      let last = chain[1].len() - 1;
      chain[1][last] ^= 1;
      chain
    })
    .await,
    case(&app, "software level outside lab mode", |c, n| {
      pki.valid_chain(&c.verifying_key(), &KeySpec::software(n))
    })
    .await,
  ];

  let unissued = random_bytes(32);
  let c = Client::new();
  let chain = strongbox(&c, &pki, &unissued);
  refused.push(("unissued nonce", c, chain, unissued));

  let expired = case(&app, "expired nonce", |c, n| strongbox(c, &pki, n)).await;
  sqlx::query("UPDATE challenges SET expires_at_unix = 1 WHERE nonce = $1")
    .bind(&expired.3)
    .execute(&app.owner_pool)
    .await
    .unwrap();
  refused.push(expired);

  for (name, client, chain, challenge) in refused {
    let (status, body) = register(&app, &client, chain, challenge).await;
    assert_eq!(
      (name, status, body.as_slice()),
      (name, StatusCode::FORBIDDEN, REFUSED)
    );
    assert_eq!(level(&app, &client).await, None, "{name}");
  }

  // Software claim without a chain stays refused outside lab mode.
  let plain = Client::new().attested_request(Vec::new(), Vec::new());
  let (status, body) = app
    .call("POST", "/v1/accounts", plain.encode_to_vec())
    .await;
  assert_eq!((status, body.as_slice()), (StatusCode::FORBIDDEN, REFUSED));
}

#[tokio::test]
async fn the_challenge_has_its_own_rate_limit_and_is_never_cached() {
  let Some((app, _)) = attest_app(|config| config.challenge_limit_per_minute = 1).await else {
    return;
  };
  let first = app.get_response("/v1/challenge").await;
  assert_eq!(first.status(), StatusCode::OK);
  assert_eq!(first.headers()["cache-control"], "no-store");
  // The registration limit is far above 1, so only the challenge limit can refuse this.
  let second = app.get_response("/v1/challenge").await;
  assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn a_database_failure_is_not_reported_as_an_unknown_challenge() {
  let Some((app, _)) = attest_app(|_| {}).await else {
    return;
  };
  let mut tx = app.pool.begin().await.unwrap();
  // Aborts the transaction, so the next statement fails at the database.
  assert!(sqlx::query("SELECT 1/0").execute(&mut *tx).await.is_err());
  let outcome = meshpay_server::ledger::challenge::consume(&mut tx, &random_bytes(32), 0).await;
  assert!(outcome.is_err(), "database errors must propagate");
}

#[tokio::test]
async fn a_challenge_is_single_use_even_after_a_failed_attempt() {
  let Some((app, pki)) = attest_app(|_| {}).await else {
    return;
  };
  let client = Client::new();
  let n = nonce(&app).await;
  let bad = pki.valid_chain(
    &client.verifying_key(),
    &KeySpec::strongbox(&random_bytes(32)),
  );
  assert_eq!(
    register(&app, &client, bad, n.clone()).await.0,
    StatusCode::FORBIDDEN
  );
  let good = pki.valid_chain(&client.verifying_key(), &KeySpec::strongbox(&n));
  assert_eq!(
    register(&app, &client, good, n.clone()).await.0,
    StatusCode::FORBIDDEN
  );

  let first = register_ok(&app, &pki, KeySpec::strongbox).await;
  assert!(level(&app, &first).await.is_some());
  // Same nonce reused by a second honest device after a success.
  let n = nonce(&app).await;
  let a = Client::new();
  let b = Client::new();
  let chain_a = pki.valid_chain(&a.verifying_key(), &KeySpec::strongbox(&n));
  let chain_b = pki.valid_chain(&b.verifying_key(), &KeySpec::strongbox(&n));
  assert_eq!(
    register(&app, &a, chain_a, n.clone()).await.0,
    StatusCode::OK
  );
  assert_eq!(
    register(&app, &b, chain_b, n).await.0,
    StatusCode::FORBIDDEN
  );
}
