use crate::http::proto_body::encode;
use crate::state::AppState;
use crate::wire::server_key_response::ServerKeyResponse;
use axum::extract::State;
use axum::response::Response;

pub async fn server_key(State(state): State<AppState>) -> Response {
  encode(&ServerKeyResponse {
    public_key: state.public_key(),
    key_id: state.key_id(),
  })
}
