//! Ingestion API (`POST /api/log`).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use greplog_engine::error::EngineError;
use greplog_engine::ingest::{IngestBatch, WalCommand};
use greplog_engine::record::{
    is_valid_duration_ms, is_valid_service_name, is_valid_span_id, is_valid_trace_id, LogRecord,
    SERVICE_NAME_MAX_LEN, SPAN_ID_MAX_LEN, TRACE_ID_MAX_LEN,
};
use tokio::sync::{mpsc, oneshot};

use crate::error::ApiError;

fn internal_error(message: impl Into<String>) -> ApiError {
    ApiError::Engine(EngineError::IoError(std::io::Error::other(message.into())))
}

/// Rejects records whose service name could alter the storage layout before
/// anything reaches the WAL. Service names become `service=<name>` partition
/// directories, so an unvalidated `../../x` would write outside the data
/// directory; the whole batch fails closed rather than skipping one record.
fn validate_batch(payload: &[LogRecord]) -> Result<(), ApiError> {
    for record in payload {
        if !is_valid_service_name(&record.service) {
            return Err(ApiError::BadRequest(format!(
                "invalid service name {:?}: use 1 to {SERVICE_NAME_MAX_LEN} characters of \
                 a-z, A-Z, 0-9, '_', '.' or '-'",
                record.service
            )));
        }
        if let Some(trace_id) = &record.trace_id {
            if !is_valid_trace_id(trace_id) {
                return Err(ApiError::BadRequest(format!(
                    "invalid trace_id {:?}: use 1 to {TRACE_ID_MAX_LEN} characters of a-z, A-Z, 0-9, '_', '.' or '-'",
                    trace_id
                )));
            }
        }
        if let Some(span_id) = &record.span_id {
            if !is_valid_span_id(span_id) {
                return Err(ApiError::BadRequest(format!(
                    "invalid span_id {:?}: use 1 to {SPAN_ID_MAX_LEN} characters of a-z, A-Z, 0-9, '_', '.' or '-'",
                    span_id
                )));
            }
        }
        if let Some(parent_span_id) = &record.parent_span_id {
            if !is_valid_span_id(parent_span_id) {
                return Err(ApiError::BadRequest(format!(
                    "invalid parent_span_id {:?}: use 1 to {SPAN_ID_MAX_LEN} characters of a-z, A-Z, 0-9, '_', '.' or '-'",
                    parent_span_id
                )));
            }
        }
        if let Some(duration_ms) = record.duration_ms {
            if !is_valid_duration_ms(duration_ms) {
                return Err(ApiError::BadRequest(format!(
                    "invalid duration_ms {duration_ms}: must be between 0 and 86400000"
                )));
            }
        }
    }
    Ok(())
}

/// Handles `POST /api/log`: forwards the batch to the WAL worker and awaits
/// the group-commit acknowledgement before returning `200 OK`.
pub async fn handle_ingest(
    State(wal_tx): State<mpsc::Sender<WalCommand>>,
    Json(payload): Json<Vec<LogRecord>>,
) -> Result<StatusCode, ApiError> {
    validate_batch(&payload)?;

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

#[cfg(test)]
mod tests {
    use super::handle_ingest;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::post;
    use axum::Router;
    use greplog_engine::ingest::WalCommand;
    use tokio::sync::mpsc;
    use tower::ServiceExt;

    /// Router backed by a stub WAL worker that acknowledges every batch,
    /// so handler behavior is observable end to end.
    fn app_with_stub_worker() -> Router {
        let (wal_tx, mut wal_rx) = mpsc::channel::<WalCommand>(8);
        tokio::spawn(async move {
            while let Some(WalCommand::Append(batch)) = wal_rx.recv().await {
                let _ = batch.responder.send(Ok(()));
            }
        });
        Router::new()
            .route("/api/log", post(handle_ingest))
            .with_state(wal_tx)
    }

    async fn post_records(app: Router, body: serde_json::Value) -> StatusCode {
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/log")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("build request"),
            )
            .await
            .expect("handler must respond");
        response.status()
    }

    #[tokio::test]
    async fn valid_batches_are_acknowledged() {
        let status = post_records(
            app_with_stub_worker(),
            serde_json::json!([{
                "timestamp_us": 1_723_000_000_123_456_i64,
                "level": "INFO",
                "service": "auth-api",
                "message": "hello"
            }]),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn traversal_service_names_are_rejected_before_the_wal() {
        let status = post_records(
            app_with_stub_worker(),
            serde_json::json!([{
                "timestamp_us": 1,
                "level": "INFO",
                "service": "../evil",
                "message": "escape attempt"
            }]),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn one_bad_record_fails_the_whole_batch() {
        let status = post_records(
            app_with_stub_worker(),
            serde_json::json!([
                {"timestamp_us": 1, "level": "INFO", "service": "fine", "message": "a"},
                {"timestamp_us": 2, "level": "INFO", "service": "no/slashes", "message": "b"}
            ]),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn invalid_trace_ids_are_rejected() {
        let status = post_records(
            app_with_stub_worker(),
            serde_json::json!([{
                "timestamp_us": 1,
                "level": "INFO",
                "service": "auth-api",
                "message": "ok",
                "trace_id": "bad/id"
            }]),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn invalid_durations_are_rejected() {
        let status = post_records(
            app_with_stub_worker(),
            serde_json::json!([{
                "timestamp_us": 1,
                "level": "INFO",
                "service": "auth-api",
                "message": "ok",
                "duration_ms": -1
            }]),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let status = post_records(
            app_with_stub_worker(),
            serde_json::json!([{
                "timestamp_us": 1,
                "level": "INFO",
                "service": "auth-api",
                "message": "ok",
                "duration_ms": 86400001
            }]),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn traced_rows_are_accepted() {
        let status = post_records(
            app_with_stub_worker(),
            serde_json::json!([{
                "timestamp_us": 1,
                "level": "INFO",
                "service": "auth-api",
                "message": "ok",
                "trace_id": "job_abc-123",
                "span_id": "span_1",
                "parent_span_id": "parent_0",
                "duration_ms": 42
            }]),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
}
