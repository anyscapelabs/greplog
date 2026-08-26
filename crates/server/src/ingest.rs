//! Ingestion API (`POST /api/log`).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use greplog_engine::error::EngineError;
use greplog_engine::ingest::{IngestBatch, WalCommand};
use greplog_engine::record::LogRecord;
use tokio::sync::{mpsc, oneshot};

use crate::error::ApiError;

fn internal_error(message: impl Into<String>) -> ApiError {
    ApiError::Engine(EngineError::IoError(std::io::Error::other(message.into())))
}

/// Handles `POST /api/log`: forwards the batch to the WAL worker and awaits
/// the group-commit acknowledgement before returning `200 OK`.
pub async fn handle_ingest(
    State(wal_tx): State<mpsc::Sender<WalCommand>>,
    Json(payload): Json<Vec<LogRecord>>,
) -> Result<StatusCode, ApiError> {
    let (responder, rx) = oneshot::channel();
    wal_tx
        .send(WalCommand::Append(IngestBatch::new(payload, responder)))
        .await
        .map_err(|e| internal_error(e.to_string()))?;

    rx.await
        .map_err(|_| internal_error("wal worker dropped responder"))?
        .map_err(ApiError::Engine)?;

    Ok(StatusCode::OK)
}
