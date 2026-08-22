//! API error mapping: engine detail is logged server-side; clients get a
//! stable code and a safe message.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use greplog_engine::error::EngineError;
use serde::Serialize;

/// JSON body returned for every error response.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
}

/// Errors that can be returned by the HTTP API.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("engine error: {0}")]
    Engine(#[from] EngineError),

    #[error("bad request: {0}")]
    BadRequest(String),
}

impl ApiError {
    /// Maps to a status code and a client-safe body; engine internals never
    /// reach the wire.
    fn into_parts(self) -> (StatusCode, ErrorBody) {
        match &self {
            Self::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    code: "bad_request",
                    message: msg.clone(),
                },
            ),
            Self::Engine(EngineError::QueryRejected(reason)) => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    code: "query_rejected",
                    message: reason.clone(),
                },
            ),
            Self::Engine(EngineError::QueryTimeout) => (
                StatusCode::REQUEST_TIMEOUT,
                ErrorBody {
                    code: "query_timeout",
                    message: "query exceeded its time budget and was cancelled".to_string(),
                },
            ),
            Self::Engine(EngineError::LockError) => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorBody {
                    code: "service_unavailable",
                    message: "service temporarily unavailable".to_string(),
                },
            ),
            Self::Engine(err) => {
                tracing::error!(error = %err, "internal engine error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorBody {
                        code: "internal_error",
                        message: "internal server error".to_string(),
                    },
                )
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = self.into_parts();
        (status, Json(body)).into_response()
    }
}
