use super::*;
use crate::query_engine::QueryEngine;
use crate::server::AppState;
use crate::store::Writer;
use axum::body::Body;
use axum::Router;
use greplog_core::gen::{IngestBatch, IngestResponse, LogEvent};
use prost::Message;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tower::ServiceExt;

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("greplog_integration_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(dir.join(".greplog"));
    dir
}

async fn send_frame(
    stream: &mut TcpStream,
    batch: &IngestBatch,
) -> IngestResponse {
    let encoded = batch.encode_to_vec();
    let len = (encoded.len() as u32).to_le_bytes();
    stream.write_all(&len).await.unwrap();
    stream.write_all(&encoded).await.unwrap();

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.unwrap();
    let resp_len = u32::from_le_bytes(len_buf) as usize;
    let mut resp_buf = vec![0u8; resp_len];
    stream.read_exact(&mut resp_buf).await.unwrap();
    IngestResponse::decode(&resp_buf[..]).unwrap()
}

fn make_test_log(service: &str, msg: &str, level: &str, event_id: &str) -> LogEvent {
    let mut attrs = HashMap::new();
    attrs.insert("env".into(), "test".into());
    attrs.insert("key".into(), "val".into());
    LogEvent {
        service_name: service.into(),
        message: msg.into(),
        level: level.into(),
        timestamp_ns: 1_700_000_000_000_000_000,
        logger_name: "integration-test".into(),
        file: "test.rs".into(),
        line: 42,
        correlation_id: "corr-123".into(),
        attributes: attrs,
        stack_trace: vec![],
        exception_type: String::new(),
        exception_message: String::new(),
        event_id: event_id.into(),
    }
}

async fn start_agent(
    dir: &std::path::Path,
    max_buffer_events: usize,
    flush_interval_secs: u64,
) -> (
    TcpStream,
    Arc<QueryEngine>,
    Arc<AtomicUsize>,
    mpsc::Sender<(IngestBatch, bytes::Bytes, oneshot::Sender<greplog_core::gen::IngestResponse>)>,
    Router,
) {
    let tcp_port = {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    };

    let config = Arc::new(Config {
        workspace: dir.to_path_buf(),
        tcp_port,
        ui_port: 0,
        flush_interval_secs,
        socket_path: PathBuf::from(".greplog/test.sock"),
        max_buffer_events,
        compaction_interval_secs: 9999,
    });

    let (batch_tx, batch_rx) = mpsc::channel(INGEST_CHANNEL_CAPACITY);

    let writer = Writer::new(&config.workspace, config.max_buffer_events)
        .expect("new writer");
    let dropped_events = writer.dropped_events();
    writer.run_startup(50_000).expect("startup");
    let _writer_handle = writer.spawn(
        batch_rx,
        Duration::from_secs(config.flush_interval_secs),
    );

    let _ingest_handle = ingest::IngestServer::new(batch_tx.clone()).spawn(&config);
    let query_engine = Arc::new(
        QueryEngine::new(&config.workspace).expect("new query engine"),
    );
    let qe = query_engine.clone();
    let de = dropped_events.clone();
    let http_app = axum::Router::new()
        .route("/query", axum::routing::post(crate::server::query_handler))
        .with_state(Arc::new(AppState {
            config,
            query_engine: (*qe).clone(),
            dropped_events: de,
            live_tx: None,
        }));

    tokio::time::sleep(Duration::from_millis(200)).await;

    let stream = TcpStream::connect(("127.0.0.1", tcp_port))
        .await
        .expect("connect to ingest");

    (stream, query_engine, dropped_events, batch_tx, http_app)
}

#[tokio::test]
async fn test_cross_sdk_proto_contract() {
    let dir = test_dir("cross_sdk");

    let (mut stream, engine, dropped_events, batch_tx, http_app) =
        start_agent(&dir, 100_000, 1).await;

    let event_id_1 = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    let event_id_2 = "01ARZ3NDEKTW4RRFFQ69G5FAW";
    let span_event_id = "01ARZ3NDEKTV5RRFFQ69G5FAX";

    let batch = IngestBatch {
        service_name: "integration-svc".into(),
        instance_id: "instance-abc".into(),
        batch_seq: 1,
        logs: vec![
            make_test_log("integration-svc", "hello cross-sdk", "info", event_id_1),
            make_test_log("integration-svc", "second event", "warn", event_id_2),
        ],
        spans: vec![],
        metrics: vec![],
    };
    let resp = send_frame(&mut stream, &batch).await;
    assert!(resp.accepted);
    assert_eq!(resp.events_count, 2);

    let span = greplog_core::gen::Span {
        service_name: "integration-svc".into(),
        name: "GET /test".into(),
        start_time_ns: 1_700_000_000_000_000_000,
        end_time_ns: 1_700_000_000_000_001_000,
        correlation_id: "corr-456".into(),
        parent_correlation_id: String::new(),
        route: "/test".into(),
        method: "GET".into(),
        status_code: 200,
        error: String::new(),
        attributes: [("span_key".into(), "span_val".into())].into(),
        kind: 2,
        event_id: span_event_id.into(),
    };
    let batch2 = IngestBatch {
        service_name: "integration-svc".into(),
        instance_id: "instance-abc".into(),
        batch_seq: 2,
        logs: vec![],
        spans: vec![span],
        metrics: vec![],
    };
    let resp2 = send_frame(&mut stream, &batch2).await;
    assert!(resp2.accepted);
    assert_eq!(resp2.events_count, 1);

    tokio::time::sleep(Duration::from_millis(1500)).await;

    let log_result = engine
        .query("SELECT message, level, id, service, attributes FROM logs ORDER BY id")
        .await
        .expect("log query");
    assert_eq!(log_result.row_count, 2);

    assert_eq!(log_result.rows[0][0], "hello cross-sdk");
    assert_eq!(log_result.rows[0][1], "info");
    assert_eq!(log_result.rows[0][2], event_id_1);
    assert_eq!(log_result.rows[0][3], "integration-svc");
    let attrs: String = serde_json::from_value(log_result.rows[0][4].clone()).unwrap();
    assert!(attrs.contains("\"env\":\"test\"") || attrs.contains("env"));

    assert_eq!(log_result.rows[1][0], "second event");
    assert_eq!(log_result.rows[1][1], "warn");
    assert_eq!(log_result.rows[1][2], event_id_2);

    let span_result = engine
        .query("SELECT name, id, method, status_code FROM spans ORDER BY id")
        .await
        .expect("span query");
    assert_eq!(span_result.row_count, 1);
    assert_eq!(span_result.rows[0][0], "GET /test");
    assert_eq!(span_result.rows[0][1], span_event_id);
    assert_eq!(span_result.rows[0][2], "GET");
    assert_eq!(span_result.rows[0][3], 200i64);

    let http_resp = http_app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method(axum::http::Method::POST)
                .uri("/query")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"sql":"SELECT count(*) AS cnt FROM logs"}"#))
                .unwrap(),
        )
        .await
        .expect("HTTP query");
    assert_eq!(http_resp.status(), 200);
    let body_bytes = axum::body::to_bytes(http_resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body["row_count"], 1);
    let cnt = body["rows"][0][0].as_i64().unwrap_or(0);
    assert!(cnt >= 2, "expected >= 2 logs, got {cnt}");

    assert_eq!(dropped_events.load(Ordering::SeqCst), 0);

    drop(batch_tx);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_shipped_defaults_smoke() {
    let dir = test_dir("shipped_defaults");

    let (mut stream, engine, dropped_events, batch_tx, _http_app) =
        start_agent(&dir, 50_000, 2).await;

    let n_events = 10_000usize;
    let mut total_accepted = 0i64;

    for chunk_start in (0..n_events).step_by(500) {
        let chunk_end = (chunk_start + 500).min(n_events);
        let logs: Vec<LogEvent> = (chunk_start..chunk_end)
            .map(|i| {
                let mut attrs = HashMap::new();
                attrs.insert("idx".into(), i.to_string());
                LogEvent {
                    service_name: "smoke-svc".into(),
                    message: format!("event-{i}"),
                    level: "info".into(),
                    timestamp_ns: 1_700_000_000_000_000_000 + i as i64,
                    logger_name: "smoke-test".into(),
                    file: "lib.rs".into(),
                    line: (i % 1000) as i32,
                    correlation_id: String::new(),
                    attributes: attrs,
                    stack_trace: vec![],
                    exception_type: String::new(),
                    exception_message: String::new(),
                    event_id: format!("evt-{:020}", i),
                }
            })
            .collect();

        let batch = IngestBatch {
            service_name: "smoke-svc".into(),
            instance_id: "smoke-instance".into(),
            batch_seq: (chunk_start / 500) as i64,
            logs,
            spans: vec![],
            metrics: vec![],
        };

        let resp = send_frame(&mut stream, &batch).await;
        if resp.accepted {
            total_accepted += resp.events_count;
        } else {
            panic!(
                "batch rejected at event {}: {}",
                chunk_start, resp.error
            );
        }
    }

    assert_eq!(total_accepted, n_events as i64);

    tokio::time::sleep(Duration::from_millis(3000)).await;

    assert_eq!(dropped_events.load(Ordering::SeqCst), 0, "no drops");

    let result = engine
        .query("SELECT count(*) AS cnt FROM logs WHERE service = 'smoke-svc'")
        .await
        .expect("count query");
    assert_eq!(
        result.rows[0][0],
        serde_json::json!(n_events as i64),
        "all {n_events} events queryable after flush"
    );

    let sample = engine
        .query("SELECT message FROM logs WHERE message = 'event-0' LIMIT 1")
        .await
        .expect("sample query");
    assert_eq!(sample.row_count, 1);
    assert_eq!(sample.rows[0][0], "event-0");

    drop(batch_tx);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = fs::remove_dir_all(&dir);
}
