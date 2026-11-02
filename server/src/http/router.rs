use crate::http::{accounts, chequebooks, deposits, health, server_key};
use crate::state::AppState;
use axum::Router;
use axum::error_handling::HandleErrorLayer;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::routing::{get, post};
use std::time::Duration;
use tower::{BoxError, ServiceBuilder};

pub fn router(state: AppState) -> Router {
  let timeout = Duration::from_secs(state.config.request_timeout_seconds);
  let concurrency = state.config.max_concurrent_requests;
  Router::new()
    .route("/healthz", get(health::health))
    .route("/v1/server-key", get(server_key::server_key))
    .route("/v1/accounts", post(accounts::register))
    .route(
      "/v1/accounts/{account_id}/balance",
      get(accounts::get_balance),
    )
    .route("/v1/chequebooks", post(chequebooks::issue))
    .route("/v1/deposits", post(deposits::deposit))
    .layer(DefaultBodyLimit::max(64 * 1024))
    .layer(
      ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|_: BoxError| async {
          StatusCode::SERVICE_UNAVAILABLE
        }))
        .timeout(timeout)
        .concurrency_limit(concurrency),
    )
    .with_state(state)
}
