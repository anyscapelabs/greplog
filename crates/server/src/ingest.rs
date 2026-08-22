//! Ingestion API (`POST /api/log`).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use greplog_engine::ingest::{IngestBatch, WalCommand};
use greplog_engine::record::LogRecord;
use tokio::sync::{mpsc, oneshot};

use crate::error::ApiError;

/// Handles `POST /api/log`: forwards the batch to the WAL worker and awaits
/// the group-commit acknowledgement before returning `200 OK`.
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
