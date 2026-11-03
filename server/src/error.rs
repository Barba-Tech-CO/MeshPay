use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub enum ApiError {
  BadRequest(String),
  Forbidden(String),
  NotFound(String),
  Unprocessable(String),
  TooManyRequests(String),
  Internal(String),
}

impl ApiError {
  pub fn forbidden(message: impl Into<String>) -> Self {
    Self::Forbidden(message.into())
  }
}

impl From<sqlx::Error> for ApiError {
  fn from(error: sqlx::Error) -> Self {
    // 23514 = check_violation: a database invariant (balance, supply, chequebook shape) refused it.
    let code = error.as_database_error().and_then(|e| e.code());
    if code.as_deref() == Some("23514") {
      tracing::warn!(%error, "ledger invariant refused a write");
      return Self::Unprocessable("refused by ledger invariant".to_owned());
    }
    Self::Internal(error.to_string())
  }
}

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    let (status, message) = match self {
      Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
      Self::Forbidden(m) => (StatusCode::FORBIDDEN, m),
      Self::NotFound(m) => (StatusCode::NOT_FOUND, m),
      Self::Unprocessable(m) => (StatusCode::UNPROCESSABLE_ENTITY, m),
      Self::TooManyRequests(m) => (StatusCode::TOO_MANY_REQUESTS, m),
      Self::Internal(detail) => {
        tracing::error!(%detail, "internal error");
        (
          StatusCode::INTERNAL_SERVER_ERROR,
          "internal error".to_owned(),
        )
      }
    };
    (status, message).into_response()
  }
}
