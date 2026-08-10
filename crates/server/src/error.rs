//! API error mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use greplog_engine::error::EngineError;

/// Errors that can be returned by the HTTP API.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// An error originating from the `greplog-engine`.
    #[error("engine error: {0}")]
    Engine(#[from] EngineError),

    /// The client sent a malformed request.
    #[error("bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::Engine(EngineError::LockError) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.to_string())
            }
            Self::Engine(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        (status, message).into_response()
    }
}
