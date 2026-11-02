use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use meshpay_server::config::app_config::AppConfig;
use meshpay_server::crypto::p256_keys::{
  compressed_public_key, key_id, sign, signing_key_from_seed,
};
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
  router: Router,
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
  let mut config = AppConfig {
    database_url: database_url.clone(),
    listen_addr: "127.0.0.1:0".to_owned(),
    lab_mode: true,
    server_signing_key_hex: hex::encode(random_bytes(32)),
    cap_cheque_units: 1_000,
    cap_reserve_units: 5_000,
    cheque_max_count: 10,
    cheque_ttl_max_seconds: 3_600,
    balance_auth_window_seconds: 60,
    request_timeout_seconds: 10,
    max_concurrent_requests: 64,
    register_limit_per_minute: 10_000,
  };
  tweak(&mut config);
  pool::migrate(&owner_url, 1_000_000_000_000, 10_000, &runtime_password)
    .await
    .unwrap();
  let owner_pool = pool::connect(&owner_url).await.unwrap();
  let grant: i64 = sqlx::query_scalar("SELECT initial_grant FROM supply")
    .fetch_one(&owner_pool)
    .await
    .unwrap();
  assert_eq!(
    grant, 10_000,
    "the tests expect INITIAL_GRANT_UNITS=10000 in .env"
  );
  let pool = pool::connect(&database_url).await.unwrap();
  let state = AppState::new(pool.clone(), config).unwrap();
  Some(TestApp {
    pool,
    owner_pool,
    router: router(state.clone()),
    state,
  })
}

impl TestApp {
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

  pub async fn register(&self, app: &TestApp) -> (StatusCode, Option<RegisterResponse>) {
    let public_key = compressed_public_key(&self.key);
    let signature = sign(&self.key, &register_message(&public_key));
    let request = RegisterRequest {
      public_key,
      signature,
      attestation_level: AttestationLevel::LabSoftware as i32,
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
