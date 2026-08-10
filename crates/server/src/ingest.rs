//! Ingestion API (`POST /api/log` on port 5050).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use greplog_engine::ingest::{IngestBatch, WalCommand};
use greplog_engine::record::LogRecord;
use tokio::sync::{mpsc, oneshot};

use crate::error::ApiError;

/// Handles `POST /api/log`.
///
/// Receives a batch of `LogRecord`s, forwards it to the WAL worker via the
/// MPSC channel, and awaits the `oneshot` group-commit acknowledgement before
/// returning `200 OK`.
///
/// # Errors
///
/// Returns `ApiError::Engine` if the WAL channel is closed or the `oneshot`
/// acknowledgement fails, and `ApiError::BadRequest` for malformed payloads.
pub async fn handle_ingest(
    State(wal_tx): State<mpsc::Sender<WalCommand>>,
    Json(payload): Json<Vec<LogRecord>>,
) -> Result<StatusCode, ApiError> {
    let (responder, rx) = oneshot::channel();
    let batch = IngestBatch::new(payload, responder);

    wal_tx
        .send(WalCommand::Append(batch))
        .await
        .map_err(|e| ApiError::Engine(greplog_engine::error::EngineError::IoError(std::io::Error::other(e.to_string()))))?;

    rx.await
        .map_err(|_| {
            ApiError::Engine(greplog_engine::error::EngineError::IoError(std::io::Error::other(
                "wal worker dropped responder",
            )))
        })?
        .map_err(ApiError::Engine)?;

    Ok(StatusCode::OK)
}
