use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::Router;
use greplog_engine::ingest::WalCommand;
use greplog_server::ingest::handle_ingest;
use serde_json::json;
use tokio::sync::mpsc;
use tower::ServiceExt;

#[tokio::test]
async fn ingest_endpoint_returns_200_ok() {
    // Create a WAL channel and a mock worker that ACKs every append.
    let (wal_tx, mut wal_rx) = mpsc::channel::<WalCommand>(8);

    tokio::spawn(async move {
        while let Some(cmd) = wal_rx.recv().await {
            if let WalCommand::Append(batch) = cmd {
                let _ = batch.responder.send(Ok(()));
            }
        }
    });

    let app = Router::new()
        .route("/api/log", post(handle_ingest))
        .with_state(wal_tx);

    let payload = json!([
        {
            "timestamp_us": 1_700_000_000_000_123_i64,
            "trace_id": "abc",
            "level": "INFO",
            "service": "test-service",
            "message": "hello",
            "raw_body": null
        }
    ]);

    let request = Request::builder()
        .method("POST")
        .uri("/api/log")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .expect("build request");

    let response = app.oneshot(request).await.expect("service must respond");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "POST /api/log must return 200 OK"
    );
}
