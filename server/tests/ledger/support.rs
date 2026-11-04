use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use meshpay_server::config::app_config::AppConfig;
use meshpay_server::crypto::p256_keys::{
  compressed_public_key, key_id, sign, signing_key_from_seed,
};
use meshpay_server::crypto::server_key_cert::issue_cert;
use meshpay_server::crypto::signing_messages::{
  balance_message, issue_message, register_message, spend_signing_message,
};
use meshpay_server::crypto::spend_hashing::derive_tx_id;
use meshpay_server::db::pool;
use meshpay_server::http::router::router;
use meshpay_server::state::AppState;
use meshpay_server::wire::attestation_level::AttestationLevel;
use meshpay_server::wire::cheque_spend::ChequeSpend;
use meshpay_server::wire::cheque_spend_body::ChequeSpendBody;
use meshpay_server::wire::cheque_stub::ChequeStub;
use meshpay_server::wire::deposit_request::DepositRequest;
use meshpay_server::wire::deposit_response::DepositResponse;
use meshpay_server::wire::issue_request::IssueRequest;
use meshpay_server::wire::issue_request_body::IssueRequestBody;
use meshpay_server::wire::issue_response::IssueResponse;
use meshpay_server::wire::register_request::RegisterRequest;
use meshpay_server::wire::register_response::RegisterResponse;
use meshpay_server::wire::server_key_cert::ServerKeyCert;
use meshpay_server::wire::server_key_role::ServerKeyRole;
use p256::ecdsa::SigningKey;
use prost::Message;
use sqlx::PgPool;
use tower::ServiceExt;

pub struct TestApp {
  /// Runtime role, as the server uses it.
  pub pool: PgPool,
  /// Owner role, only for checking that triggers also stop the owner.
  pub owner_pool: PgPool,
  pub state: AppState,
  /// TEST ONLY offline root of this app; signs `ServerKeyCert`s and kill-switch snapshots.
  pub root_key: SigningKey,
  /// (admin URL of the shared database, name) of a database created by `setup_isolated`.
  isolated_db: Option<(String, String)>,
  router: Router,
}

pub fn unix_now() -> u64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs()
}

pub fn random_bytes(len: usize) -> Vec<u8> {
  let mut bytes = vec![0u8; len];
  getrandom::fill(&mut bytes).unwrap();
  bytes
}

pub async fn setup(lab_mode: bool) -> Option<TestApp> {
  setup_with(|config| config.lab_mode = lab_mode).await
}

/// `None` (test skipped) when the database env is unset, so CI without a database does not fail.
pub async fn setup_with(tweak: impl FnOnce(&mut AppConfig)) -> Option<TestApp> {
  setup_inner(tweak, None, None).await
}

/// Shared-database app whose own ISSUER cert is valid only in `not_before..=not_after`.
pub async fn setup_with_cert(not_before: u64, not_after: u64) -> Option<TestApp> {
  setup_inner(|_| {}, None, Some((not_before, not_after))).await
}

/// App on a fresh database of its own (supply cap and grant chosen here), for tests that change
/// global state (kill switch) or tamper with append-only rows. Call `drop_isolated` at the end.
pub async fn setup_isolated(supply_cap: u64, initial_grant: u64) -> Option<TestApp> {
  setup_inner(|_| {}, Some((supply_cap, initial_grant)), None).await
}

pub async fn setup_isolated_with(
  supply_cap: u64,
  initial_grant: u64,
  tweak: impl FnOnce(&mut AppConfig),
) -> Option<TestApp> {
  setup_inner(tweak, Some((supply_cap, initial_grant)), None).await
}

/// Same URL on another database (the URLs end with `/<database>`).
fn on_database(url: &str, database: &str) -> String {
  format!("{}/{database}", url.rsplit_once('/').unwrap().0)
}

async fn setup_inner(
  tweak: impl FnOnce(&mut AppConfig),
  isolated: Option<(u64, u64)>,
  cert_window: Option<(u64, u64)>,
) -> Option<TestApp> {
  let _ = tracing_subscriber::fmt().with_test_writer().try_init();
  let env = |name: &str| std::env::var(name).ok();
  let (Some(database_url), Some(owner_url), Some(runtime_password)) = (
    env("DATABASE_URL"),
    env("MIGRATOR_DATABASE_URL"),
    env("RUNTIME_DB_PASSWORD"),
  ) else {
    eprintln!(
      "SKIPPED: DATABASE_URL, MIGRATOR_DATABASE_URL or RUNTIME_DB_PASSWORD is unset; \
       start the compose Postgres and export .env"
    );
    return None;
  };
  // TEST ONLY root and server keys, fresh per app.
  let root_key = signing_key_from_seed(&random_bytes(32)).unwrap();
  let server_seed = random_bytes(32);
  let server_key = signing_key_from_seed(&server_seed).unwrap();
  // From an hour back, so tests can backdate chequebooks within the cert.
  let now = unix_now();
  let (not_before, not_after) = cert_window.unwrap_or((now - 3_600, now + 86_400));
  let cert = issue_cert(
    &root_key,
    &server_key,
    ServerKeyRole::Issuer,
    not_before,
    not_after,
  );
  let snapshot_seed = random_bytes(32);
  let snapshot_cert = issue_cert(
    &root_key,
    &signing_key_from_seed(&snapshot_seed).unwrap(),
    ServerKeyRole::Snapshot,
    now - 3_600,
    now + 86_400,
  );
  let (database_url, owner_url, isolated_db) = match isolated {
    None => (database_url, owner_url, None),
    Some(_) => {
      let name = format!("meshpay_test_{}", hex::encode(random_bytes(8)));
      (
        on_database(&database_url, &name),
        on_database(&owner_url, &name),
        Some((owner_url, name)),
      )
    }
  };
  let mut config = test_config(
    database_url.clone(),
    &server_seed,
    &root_key,
    &cert,
    &snapshot_seed,
    &snapshot_cert,
  );
  tweak(&mut config);
  let shared_owner_url = isolated_db.as_ref().map_or(&owner_url, |(url, _)| url);
  pool::migrate(
    shared_owner_url,
    1_000_000_000_000,
    10_000,
    &runtime_password,
  )
  .await
  .unwrap();
  if let (Some((admin_url, name)), Some((cap, grant))) = (&isolated_db, isolated) {
    let admin = pool::connect(admin_url).await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
      .execute(&admin)
      .await
      .unwrap();
    admin.close().await;
    pool::apply_schema(&owner_url, cap, grant).await.unwrap();
  }
  let owner_pool = pool::connect(&owner_url).await.unwrap();
  let grant: i64 = sqlx::query_scalar("SELECT initial_grant FROM supply")
    .fetch_one(&owner_pool)
    .await
    .unwrap();
  assert!(
    isolated.is_some() || grant == 10_000,
    "the tests expect INITIAL_GRANT_UNITS=10000 in .env"
  );
  let pool = pool::connect(&database_url).await.unwrap();
  let state = AppState::new(pool.clone(), config).unwrap();
  Some(TestApp {
    pool,
    owner_pool,
    root_key,
    isolated_db,
    router: router(state.clone()),
    state,
  })
}

impl TestApp {
  pub async fn drop_isolated(self) {
    let Some((admin_url, name)) = self.isolated_db else {
      return;
    };
    self.pool.close().await;
    self.owner_pool.close().await;
    self.state.pool.close().await;
    let admin = pool::connect(&admin_url).await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
      "DROP DATABASE {name} WITH (FORCE)"
    )))
    .execute(&admin)
    .await
    .unwrap();
  }

  pub async fn call(&self, method: &str, path: &str, body: Vec<u8>) -> (StatusCode, Vec<u8>) {
    let request = Request::builder()
      .method(method)
      .uri(path)
      .body(Body::from(body))
      .unwrap();
    let response = self.router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
  }

  /// Full response, for tests that look at headers.
  pub async fn get_response(&self, path: &str) -> axum::response::Response {
    let request = Request::builder().uri(path).body(Body::empty()).unwrap();
    self.router.clone().oneshot(request).await.unwrap()
  }

  pub async fn get_signed(
    &self,
    path: &str,
    timestamp: u64,
    signature: &[u8],
  ) -> (StatusCode, Vec<u8>) {
    let request = Request::builder()
      .uri(path)
      .header("x-meshpay-timestamp", timestamp.to_string())
      .header("x-meshpay-signature", hex::encode(signature))
      .body(Body::empty())
      .unwrap();
    let response = self.router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    (
      status,
      response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec(),
    )
  }

  pub async fn balance(&self, account_id: &[u8]) -> (i64, i64) {
    sqlx::query_as("SELECT available, reserved FROM balances WHERE account_id = $1")
      .bind(account_id)
      .fetch_one(&self.pool)
      .await
      .unwrap()
  }

  pub async fn deposit(&self, stub: &ChequeStub, spend: &ChequeSpend) -> DepositResponse {
    let request = DepositRequest {
      stub: Some(stub.clone()),
      spend: Some(spend.clone()),
    };
    let (status, bytes) = self
      .call("POST", "/v1/deposits", request.encode_to_vec())
      .await;
    assert_eq!(status, StatusCode::OK);
    DepositResponse::decode(bytes.as_slice()).unwrap()
  }
}

pub struct Client {
  key: SigningKey,
  pub account_id: Vec<u8>,
}

impl Client {
  pub fn new() -> Self {
    let key = signing_key_from_seed(&random_bytes(32)).unwrap();
    let account_id = key_id(&compressed_public_key(&key));
    Self { key, account_id }
  }

  /// Raw private scalar, only to check that proofs never carry it.
  pub fn secret_bytes(&self) -> Vec<u8> {
    self.key.to_bytes().to_vec()
  }

  pub fn public_key(&self) -> Vec<u8> {
    compressed_public_key(&self.key)
  }

  pub fn verifying_key(&self) -> p256::ecdsa::VerifyingKey {
    *self.key.verifying_key()
  }

  /// Registration with an attestation chain; the level claim is deliberately LAB_SOFTWARE.
  pub fn attested_request(&self, chain: Vec<Vec<u8>>, challenge: Vec<u8>) -> RegisterRequest {
    let public_key = compressed_public_key(&self.key);
    RegisterRequest {
      signature: sign(&self.key, &register_message(&public_key)),
      public_key,
      attestation_level: AttestationLevel::LabSoftware as i32,
      attestation_chain: chain,
      challenge,
    }
  }

  pub async fn register(&self, app: &TestApp) -> (StatusCode, Option<RegisterResponse>) {
    let public_key = compressed_public_key(&self.key);
    let signature = sign(&self.key, &register_message(&public_key));
    let request = RegisterRequest {
      public_key,
      signature,
      attestation_level: AttestationLevel::LabSoftware as i32,
      attestation_chain: Vec::new(),
      challenge: Vec::new(),
    };
    let (status, bytes) = app
      .call("POST", "/v1/accounts", request.encode_to_vec())
      .await;
    (
      status,
      RegisterResponse::decode(bytes.as_slice())
        .ok()
        .filter(|_| status == StatusCode::OK),
    )
  }

  pub async fn issue(
    &self,
    app: &TestApp,
    request_id: Vec<u8>,
    cheque_count: u32,
    max_amount_units: u64,
    ttl_seconds: u64,
  ) -> (StatusCode, Option<IssueResponse>) {
    let body = IssueRequestBody {
      account_id: self.account_id.clone(),
      request_id,
      cheque_count,
      max_amount_units,
      ttl_seconds,
    }
    .encode_to_vec();
    let signature = sign(&self.key, &issue_message(&body));
    let request = IssueRequest { body, signature };
    let (status, bytes) = app
      .call("POST", "/v1/chequebooks", request.encode_to_vec())
      .await;
    (
      status,
      IssueResponse::decode(bytes.as_slice())
        .ok()
        .filter(|_| status == StatusCode::OK),
    )
  }

  /// Balance read of `target`, signed by this client's key at `timestamp`.
  pub async fn read_balance(&self, app: &TestApp, target: &[u8], timestamp: u64) -> StatusCode {
    let message = balance_message(target, timestamp);
    let path = format!("/v1/accounts/{}/balance", hex::encode(target));
    app
      .get_signed(&path, timestamp, &sign(&self.key, &message))
      .await
      .0
  }

  pub fn sign_raw(&self, message: &[u8]) -> Vec<u8> {
    sign(&self.key, message)
  }

  pub fn spend(&self, stub: &ChequeStub, payee: &[u8], amount: u64, counter: u64) -> ChequeSpend {
    let stub_body =
      meshpay_server::wire::cheque_stub_body::ChequeStubBody::decode(stub.body.as_slice()).unwrap();
    let mut body = ChequeSpendBody {
      cheque_id: stub_body.cheque_id,
      payee_account_id: payee.to_vec(),
      amount_units: amount,
      tx_id: Vec::new(),
      counter,
      prev_spend_hash: Vec::new(),
      signed_at_unix: 1,
    };
    body.tx_id = derive_tx_id(counter, &body.encode_to_vec()).unwrap();
    let body_bytes = body.encode_to_vec();
    ChequeSpend {
      payer_signature: sign(&self.key, &spend_signing_message(&body_bytes)),
      body: body_bytes,
    }
  }
}

fn test_config(
  database_url: String,
  server_seed: &[u8],
  root_key: &SigningKey,
  cert: &ServerKeyCert,
  snapshot_seed: &[u8],
  snapshot_cert: &ServerKeyCert,
) -> AppConfig {
  AppConfig {
    database_url,
    listen_addr: "127.0.0.1:0".to_owned(),
    lab_mode: true,
    server_signing_key_hex: hex::encode(server_seed),
    server_root_public_key_hex: hex::encode(compressed_public_key(root_key)),
    server_key_cert_hex: hex::encode(cert.encode_to_vec()),
    server_snapshot_key_hex: hex::encode(snapshot_seed),
    server_snapshot_key_cert_hex: hex::encode(snapshot_cert.encode_to_vec()),
    cap_cheque_units: 1_000,
    cap_reserve_units: 5_000,
    cheque_max_count: 10,
    cheque_ttl_max_seconds: 3_600,
    balance_auth_window_seconds: 60,
    request_timeout_seconds: 10,
    max_concurrent_requests: 64,
    register_limit_per_minute: 10_000,
    challenge_limit_per_minute: 10_000,
    control_limit_per_minute: 10_000,
    public_read_limit_per_minute: 10_000,
    attestation_roots_pem_file: "/dev/null".to_owned(),
    attestation_revocation_file: "/dev/null".to_owned(),
    attestation_challenge_ttl_seconds: 60,
    allow_tee_issuance: false,
  }
}

/// `AppState::new` (no database touched) with ISSUER / SNAPSHOT certs valid in the given windows;
/// `shared_key` reuses the issuer seed as the SNAPSHOT key.
pub fn app_state_for(
  issuer: (u64, u64),
  snapshot: (u64, u64),
  shared_key: bool,
) -> Result<AppState, String> {
  let root_key = signing_key_from_seed(&random_bytes(32)).unwrap();
  let server_seed = random_bytes(32);
  let snapshot_seed = if shared_key {
    server_seed.clone()
  } else {
    random_bytes(32)
  };
  let cert = issue_cert(
    &root_key,
    &signing_key_from_seed(&server_seed).unwrap(),
    ServerKeyRole::Issuer,
    issuer.0,
    issuer.1,
  );
  let snapshot_cert = issue_cert(
    &root_key,
    &signing_key_from_seed(&snapshot_seed).unwrap(),
    ServerKeyRole::Snapshot,
    snapshot.0,
    snapshot.1,
  );
  let url = "postgres://unused@127.0.0.1/unused".to_owned();
  let pool = sqlx::postgres::PgPoolOptions::new()
    .connect_lazy(&url)
    .unwrap();
  AppState::new(
    pool,
    test_config(
      url,
      &server_seed,
      &root_key,
      &cert,
      &snapshot_seed,
      &snapshot_cert,
    ),
  )
}
