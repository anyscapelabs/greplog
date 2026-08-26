//! Integration tests for the hardened `POST /api/query` surface.
//!
//! Verifies that non-`SELECT` statements are rejected before execution, that a
//! runaway query is cancelled by the timeout, and that engine errors never leak
//! internal detail to the response body.

use std::sync::Arc;

use arrow::array::RecordBatch;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::Router;
use greplog_engine::config::{EngineConfig, LiveBuffer};
use greplog_engine::memtable::MemTable;
use greplog_engine::query::QueryEngine;
use greplog_engine::record::LogRecord;
use greplog_server::dashboard::handle_query;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

fn sample(index: usize, service: &str) -> LogRecord {
    LogRecord {
        timestamp_us: 1_700_000_000_000_000 + i64::try_from(index).expect("fit i64"),
        trace_id: Some(format!("trace-{index}")),
        level: "ERROR".to_string(),
        service: service.to_string(),
        message: format!("order {index} failed"),
        raw_body: Some(r#"{"order_id": 9876}"#.to_string()),
    }
}

fn live_batch(count: usize, service: &str) -> RecordBatch {
    let mut table = MemTable::new(count);
    for index in 0..count {
        table.append_record(&sample(index, service));
    }
    table.finish().expect("finish live memtable")
}

async fn query_app(config: EngineConfig, buffer: LiveBuffer) -> Router {
    let engine = QueryEngine::new(&config, buffer)
        .await
        .expect("build query engine");
    Router::new()
        .route("/api/query", post(handle_query))
        .with_state(Arc::new(engine))
}

fn config_with(dir: &tempfile::TempDir) -> EngineConfig {
    EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    }
}

async fn post_query(app: &Router, sql: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/api/query")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "sql": sql }).to_string()))
        .expect("build request");
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("service must respond");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read response body");
    let body: Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body)
}

#[tokio::test]
async fn query_endpoint_rejects_non_select_statements() {
    let dir = tempdir().expect("create temp dir");
    let app = query_app(config_with(&dir), LiveBuffer::default()).await;

    // Statements that parse but are not read-only SELECT/WITH queries.
    for (sql, expected_message) in [
        ("COPY logs TO '/tmp/leak'", "read-only SELECT"),
        ("INSERT INTO logs VALUES (1)", "read-only SELECT"),
        ("DELETE FROM logs", "read-only SELECT"),
        (
            "SET datafusion.execution.batch_size = 100",
            "read-only SELECT",
        ),
        ("SHOW TABLES", "read-only SELECT"),
    ] {
        let (status, body) = post_query(&app, sql).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "sql: {sql}");
        assert_eq!(body["code"], "query_rejected", "sql: {sql}");
        assert!(
            body["message"]
                .as_str()
                .expect("message is a string")
                .contains(expected_message),
            "sql: {sql} — response must explain the rejection, got: {body}"
        );
    }

    // `CREATE EXTERNAL TABLE` is refused too.
    let (status, body) =
        post_query(&app, "CREATE EXTERNAL TABLE hacked (a INT) LOCATION '/etc'").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "query_rejected");
}

#[tokio::test]
async fn query_endpoint_returns_200_for_plain_select() {
    let dir = tempdir().expect("create temp dir");
    let app = query_app(config_with(&dir), LiveBuffer::default()).await;

    let (status, body) = post_query(&app, "SELECT count(*) AS n FROM logs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body[0]["n"], json!(0), "empty engine must count zero rows");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn query_endpoint_cancels_a_runaway_query_and_returns_timeout() {
    let dir = tempdir().expect("create temp dir");
    let config = EngineConfig {
        query_timeout_secs: 1,
        ..config_with(&dir)
    };
    let buffer = LiveBuffer::default();
    buffer
        .write()
        .expect("lock live buffer")
        .push(live_batch(20_000, "auth-api"));
    let app = query_app(config, buffer).await;

    // A cross join over 20k live rows cannot finish in 1s. The engine must
    // cancel it and the client must get a stable, sanitized timeout body.
    let (status, body) = post_query(
        &app,
        "SELECT count(*) FROM logs a, logs b, logs c \
         WHERE a.timestamp_us < b.timestamp_us AND b.timestamp_us < c.timestamp_us",
    )
    .await;

    assert_eq!(status, StatusCode::REQUEST_TIMEOUT, "body: {body}");
    assert_eq!(body["code"], "query_timeout");
    assert!(
        body["message"]
            .as_str()
            .expect("message is a string")
            .contains("time budget"),
        "timeout body must be client-safe, got: {body}"
    );
}
