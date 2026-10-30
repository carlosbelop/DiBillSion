use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use dibill_proto::{ApiError, ErrorCode};

/// Every failure a handler can return, rendered as an [`ApiError`] body.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("missing or unknown bearer token")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(&'static str),
    #[error("{0}")]
    NotFound(&'static str),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    TooLarge(String),
    #[error("{0}")]
    Internal(String),
    #[error(transparent)]
    Db(#[from] tokio_postgres::Error),
    #[error(transparent)]
    Pool(#[from] deadpool_postgres::PoolError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error) = match &self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, ErrorCode::Unauthorized),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, ErrorCode::Forbidden),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, ErrorCode::NotFound),
            Self::Invalid(_) => (StatusCode::BAD_REQUEST, ErrorCode::InvalidRequest),
            Self::TooLarge(_) => (StatusCode::PAYLOAD_TOO_LARGE, ErrorCode::TooLarge),
            Self::Internal(_) | Self::Db(_) | Self::Pool(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::Internal)
            }
        };
        let message = if status == StatusCode::INTERNAL_SERVER_ERROR {
            // Details stay in the server log, not in the response.
            tracing::error!(error = %self, "request failed");
            "internal error".to_owned()
        } else {
            self.to_string()
        };
        (status, Json(ApiError { error, message })).into_response()
    }
}
