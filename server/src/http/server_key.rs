use crate::state::AppState;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};

/// Public: issuer key, root public key and the ISSUER / SNAPSHOT certs (precomputed bytes).
pub async fn server_key(State(state): State<AppState>) -> Response {
  (
    [(CONTENT_TYPE, "application/x-protobuf")],
    state.server_key_response.clone(),
  )
    .into_response()
}
