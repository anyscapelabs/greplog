//! Dashboard API (`POST /api/query` and `GET /api/tail` on port 3000).

use std::convert::Infallible;
use std::sync::Arc;

use arrow::record_batch::RecordBatch;
use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::Json;
use futures::Stream;
use greplog_engine::query::QueryEngine;
use greplog_engine::record::LogRecord;
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::error::ApiError;

/// Request body for `POST /api/query`.
#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    /// SQL query to execute against the unified `logs` view.
    pub sql: String,
}

/// Handles `POST /api/query`.
///
/// Executes the supplied SQL via `QueryEngine` and returns the result as a
/// JSON array. Heavy JSON serialization is offloaded to `spawn_blocking` to
/// avoid blocking the async runtime.
///
/// # Errors
///
/// Returns `ApiError::Engine` if the SQL execution or JSON serialization fails.
pub async fn handle_query(
    State(engine): State<Arc<QueryEngine>>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let batches: Vec<RecordBatch> = engine
        .execute_sql(&req.sql)
        .await
        .map_err(ApiError::Engine)?;

    if batches.is_empty() {
        return Ok(Json(Vec::new()));
    }

    // Offload JSON conversion to the blocking pool so large payloads do not
    // stall the `tokio` event loop.
    let json_rows = tokio::task::spawn_blocking(move || {
        let mut buf = Vec::new();
        let mut writer = arrow_json::ArrayWriter::new(&mut buf);
        let refs: Vec<&RecordBatch> = batches.iter().collect();
        writer
            .write_batches(&refs)
            .map_err(greplog_engine::error::EngineError::from)?;
        writer.finish().map_err(greplog_engine::error::EngineError::from)?;
        let values: Vec<serde_json::Value> = serde_json::from_slice(&buf)
            .map_err(|e| greplog_engine::error::EngineError::ParseError(e.to_string()))?;
        Ok::<Vec<serde_json::Value>, greplog_engine::error::EngineError>(values)
    })
    .await
    .map_err(|e| {
        ApiError::Engine(greplog_engine::error::EngineError::IoError(std::io::Error::other(
            e.to_string(),
        )))
    })?
    .map_err(ApiError::Engine)?;

    Ok(Json(json_rows))
}

/// Handles `GET /api/tail` as an SSE stream.
///
/// Each `Vec<LogRecord>` received on the broadcast channel is yielded as a
/// JSON-encoded SSE `Event`.
pub async fn handle_tail(
    State(mut broadcast_rx): State<broadcast::Receiver<Vec<LogRecord>>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = async_stream::stream! {
        loop {
            match broadcast_rx.recv().await {
                Ok(records) => {
                    let event = Event::default()
                        .json_data(&records)
                        .unwrap_or_else(|_| Event::default().data("serialization error"));
                    yield Ok(event);
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => {},
            }
        }
    };

    Sse::new(stream)
}
