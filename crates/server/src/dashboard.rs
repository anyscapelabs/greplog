//! Dashboard API: `POST /api/query` and `GET /api/tail`.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use arrow::record_batch::RecordBatch;
use axum::extract::State;
use axum::http::header;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::Stream;
use greplog_engine::error::EngineError;
use greplog_engine::query::QueryEngine;
use greplog_engine::record::LogRecord;
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::error::ApiError;
use crate::shutdown::Shutdown;

fn internal_error(message: impl std::fmt::Display) -> ApiError {
    ApiError::Engine(EngineError::IoError(std::io::Error::other(
        message.to_string(),
    )))
}

/// Encodes row batches on the blocking pool (CPU-bound, result-proportional).
async fn encoded_rows(batches: Vec<RecordBatch>) -> Result<Response, ApiError> {
    if batches.is_empty() {
        return Ok(empty_rows());
    }
    let body = tokio::task::spawn_blocking(move || encode_rows(&batches))
        .await
        .map_err(internal_error)?
        .map_err(ApiError::Engine)?;
    Ok(json_response(body))
}

const TAIL_KEEP_ALIVE_SECS: u64 = 15;

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub sql: String,
}

pub async fn handle_search(
    State(engine): State<Arc<QueryEngine>>,
    Json(search): Json<greplog_engine::search::LogSearch>,
) -> Result<Response, ApiError> {
    let batches = engine.search(&search).await.map_err(ApiError::Engine)?;
    encoded_rows(batches).await
}

pub async fn handle_stats(
    State(data_dir): State<std::path::PathBuf>,
) -> Result<Response, ApiError> {
    let snapshot = tokio::task::spawn_blocking(move || crate::stats::storage_stats(&data_dir))
        .await
        .map_err(internal_error)?;
    let body = serde_json::to_vec(&snapshot).map_err(internal_error)?;
    Ok(json_response(body))
}

pub struct TailState {
    pub records: broadcast::Receiver<Vec<LogRecord>>,
    pub shutdown: Shutdown,
}

/// Handles `POST /api/query`, returning rows as a JSON array.
///
/// Batches are encoded straight to JSON bytes: round-tripping them through
/// `serde_json::Value` would walk the payload two extra times on large
/// results while producing identical output.
pub async fn handle_query(
    State(engine): State<Arc<QueryEngine>>,
    Json(req): Json<QueryRequest>,
) -> Result<Response, ApiError> {
    let batches = engine
        .execute_sql(&req.sql)
        .await
        .map_err(ApiError::Engine)?;
    encoded_rows(batches).await
}

fn encode_rows(batches: &[RecordBatch]) -> Result<Vec<u8>, EngineError> {
    let mut buf = Vec::new();
    let mut writer = arrow_json::ArrayWriter::new(&mut buf);
    let refs: Vec<&RecordBatch> = batches.iter().collect();
    writer.write_batches(&refs)?;
    writer.finish()?;
    Ok(buf)
}

fn empty_rows() -> Response {
    json_response(b"[]".to_vec())
}

fn json_response(body: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

/// Handles `GET /api/tail` as an SSE stream of live log batches.
///
/// The shutdown signal ends the stream — without it, one open dashboard tab
/// would hold a graceful drain open forever. A subscriber that falls behind
/// the channel capacity gets an explicit `gap` event instead of silently
/// missing records.
pub async fn handle_tail(
    State(state): State<TailState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let TailState {
        mut records,
        shutdown,
    } = state;

    let stream = async_stream::stream! {
        loop {
            let received = tokio::select! {
                received = records.recv() => received,
                () = shutdown.wait() => return,
            };

            match received {
                Ok(batch) => yield Ok(logs_event(&batch)),
                Err(broadcast::error::RecvError::Closed) => return,
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "live tail subscriber lagged behind the broadcast channel");
                    yield Ok(gap_event(skipped));
                }
            }
        }
    };

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(TAIL_KEEP_ALIVE_SECS)))
}

fn logs_event(batch: &[LogRecord]) -> Event {
    match Event::default().event("logs").json_data(batch) {
        Ok(event) => event,
        Err(error) => {
            tracing::error!(?error, "failed to encode live tail batch");
            gap_event(batch.len() as u64)
        }
    }
}

fn gap_event(skipped: u64) -> Event {
    Event::default().event("gap").data(skipped.to_string())
}

#[cfg(test)]
mod tests {
    use arrow::record_batch::RecordBatch;
    use greplog_engine::memtable::MemTable;
    use greplog_engine::record::LogRecord;

    use super::encode_rows;

    fn batch(rows: usize) -> RecordBatch {
        let mut table = MemTable::new(rows);
        for index in 0..rows {
            table.append_record(&LogRecord::new("INFO", "auth-api", format!("row {index}")));
        }
        table.finish().expect("finish memtable")
    }

    #[test]
    fn encoded_rows_are_a_json_array_of_objects() {
        let encoded = encode_rows(&[batch(2)]).expect("encode");
        let parsed: serde_json::Value =
            serde_json::from_slice(&encoded).expect("output must be valid JSON");
        let rows = parsed.as_array().expect("output must be a JSON array");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["service"], "auth-api");
    }

    #[test]
    fn zero_row_batches_encode_to_an_empty_array_or_nothing() {
        let encoded = encode_rows(&[batch(0)]).expect("encode");
        if encoded.is_empty() {
            return;
        }
        let parsed: serde_json::Value = serde_json::from_slice(&encoded).expect("valid JSON");
        assert!(parsed.as_array().expect("array").is_empty());
    }
}
