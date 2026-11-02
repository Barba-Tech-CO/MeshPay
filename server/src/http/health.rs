use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::State;

pub async fn health(State(state): State<AppState>) -> Result<&'static str, ApiError> {
  sqlx::query("SELECT 1").execute(&state.pool).await?;
  Ok("ok")
}
