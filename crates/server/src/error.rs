//! API error mapping.
//!
//! The HTTP layer must never leak internal engine details to clients. Engine
//! failures are logged in full server-side and reduced to a stable error code
//! plus a generic message in the response body.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use greplog_engine::error::EngineError;
use serde::Serialize;

/// JSON body returned for every error response.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    /// Stable machine-readable code for the failure class.
    pub code: &'static str,
    /// Human-readable message safe to show to the client.
    pub message: String,
}

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

impl ApiError {
    /// Maps the error to an HTTP status code and a client-safe body.
    ///
    /// Engine errors are logged with full detail via `tracing::error!`; the
    /// response only ever carries the stable [`ErrorBody::code`] and a generic
    /// message, so DataFusion/sqlparser internals never reach the wire.
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
