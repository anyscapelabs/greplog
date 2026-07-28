use super::query_engine::QueryEngine;
use axum::{
    Json,
    extract::State,
    http::{header, Method, StatusCode},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tower::ServiceBuilder;
use tracing::info;

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    status: String,
    version: &'static str,
}

pub struct AppState {
    pub config: Arc<super::Config>,
    pub query_engine: QueryEngine,
    pub dropped_events: Arc<AtomicUsize>,
}

/// JSON request body for `POST /query`.
#[derive(Debug, Clone, Deserialize)]
pub struct QueryRequest {
    pub sql: String,
}

pub struct HttpServer;

impl HttpServer {
    pub fn new() -> Self {
        Self
    }

    pub fn spawn(self, state: AppState) -> tokio::task::JoinHandle<()> {
        let config = state.config.clone();
        let shared_state = Arc::new(state);

        tokio::spawn(async move {
            let cors = tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([header::CONTENT_TYPE, header::ACCEPT]);

            let app = Router::new()
                .route("/health", get(health_handler))
                .route("/status", get(status_handler))
                .route("/query", post(query_handler))
                .route("/detect", get(detect_handler))
                .route("/resources", get(resources_handler))
                .with_state(shared_state)
                .layer(ServiceBuilder::new().layer(cors));

            let addr = format!("127.0.0.1:{}", config.ui_port);
            let listener = match tokio::net::TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Failed to bind HTTP server at {}: {}", addr, e);
                    return;
                }
            };

            info!("HTTP server listening on {}", addr);

            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("HTTP server error: {}", e);
            }
        })
    }
}

pub(crate) async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn status_handler(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "running",
        "version": env!("CARGO_PKG_VERSION"),
        "dropped_events": state.dropped_events.load(Ordering::SeqCst),
        "buffer_occupancy": 0,  // TODO: wire real buffer occupancy in a follow‑up
    }))
}

pub(crate) async fn query_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<super::query_engine::QueryResult>, (StatusCode, String)> {
    match state.query_engine.query(&req.sql).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            format!("Query failed: {e}"),
        )),
    }
}

async fn detect_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<super::detect::Detection>> {
    let detections = super::detect::detect_workspace(&state.config.workspace);
    Json(detections.unwrap_or_default())
}

async fn resources_handler(
    State(state): State<Arc<AppState>>,
) -> Json<super::detect::system::SystemResources> {
    let resources = super::detect::system::collect_system_resources(&state.config.workspace);
    Json(resources)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query_engine::LIST_FILES_CACHE_TTL_SECS;
    use crate::store::io::write_parquet_atomic;
    use arrow::array::{StringArray, TimestampMicrosecondArray};
    use arrow::record_batch::RecordBatch;
    use axum::body::Body;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;
    use std::{fs, path::Path};
    use tower::ServiceExt;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("greplog_server_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn write_log_file(base: &Path, service: &str, date: &str, id: &str, level: &str, message: &str) {
        let dir = base
            .join("data")
            .join("table_type=logs")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        let schema = greplog_core::arrow_schema::log_schema();
        let ts = 1_700_000_000_000_000;
        let batch = RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(StringArray::from(vec![id])),
                Arc::new(StringArray::from(vec![service])),
                Arc::new(TimestampMicrosecondArray::from(vec![ts])),
                Arc::new(StringArray::from(vec![Some(level)])),
                Arc::new(StringArray::from(vec![None::<&str>])),
                Arc::new(StringArray::from(vec![Some(message)])),
                Arc::new(StringArray::from(vec![None::<&str>])),
            ],
        )
        .expect("build log batch");
        write_parquet_atomic(&dir, &batch).expect("write log parquet");
    }

    fn make_state(dir: &Path) -> Arc<AppState> {
        let engine = super::super::query_engine::QueryEngine::new(dir)
            .expect("new engine");
        Arc::new(AppState {
            config: Arc::new(super::super::Config {
                workspace: dir.to_path_buf(),
                tcp_port: 0,
                ui_port: 0,
                flush_interval_secs: 2,
                socket_path: PathBuf::from(".greplog/test.sock"),
                max_buffer_events: 50_000,
                compaction_interval_secs: 300,
            }),
            query_engine: engine,
            dropped_events: Arc::new(AtomicUsize::new(0)),
        })
    }

    // ------------------------------------------------------------------
    // Test 1: End-to-end /query via the axum Router
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_query_endpoint() {
        let dir = test_dir("query_endpoint");
        write_log_file(&dir, "api", "2024-01-15", "id-1", "info", "hello world");
        let state = make_state(&dir);

        let app = Router::new()
            .route("/query", post(query_handler))
            .with_state(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method(axum::http::Method::POST)
                    .uri("/query")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"sql": "SELECT message FROM logs"}"#))
                    .unwrap(),
            )
            .await
            .expect("request");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["row_count"], 1);
        assert_eq!(result["columns"][0], "message");
        assert_eq!(result["rows"][0][0], "hello world");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 2: New data is visible within the cache TTL window
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_new_data_visibility() {
        let dir = test_dir("new_data_visibility");
        write_log_file(&dir, "api", "2024-01-15", "id-1", "info", "first batch");
        let state = make_state(&dir);

        // Query initial data
        let result = state
            .query_engine
            .query("SELECT message FROM logs ORDER BY id")
            .await
            .expect("initial query");
        assert_eq!(result.row_count, 1);
        assert_eq!(result.rows[0][0], "first batch");

        // Write additional data AFTER the engine was created
        write_log_file(&dir, "api", "2024-01-15", "id-2", "info", "second batch");

        // Query before TTL elapses (cache hit returns stale listing).
        let result = state
            .query_engine
            .query("SELECT message FROM logs ORDER BY id")
            .await
            .expect("second query (stale)");
        assert_eq!(
            result.row_count, 1,
            "stale cache: new file not yet visible",
        );

        // Wait past the TTL plus margin.
        tokio::time::sleep(Duration::from_secs(LIST_FILES_CACHE_TTL_SECS + 1)).await;

        let result = state
            .query_engine
            .query("SELECT message FROM logs ORDER BY id")
            .await
            .expect("third query (fresh)");
        assert_eq!(
            result.row_count, 2,
            "new data should be visible after cache entry expiry",
        );
        assert_eq!(result.rows[1][0], "second batch");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 3: /status returns expected shape
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_status_endpoint() {
        let dir = test_dir("status_test");
        write_log_file(&dir, "svc", "2024-01-15", "id-1", "info", "test");
        let state = make_state(&dir);

        let app = Router::new()
            .route("/status", get(status_handler))
            .with_state(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method(axum::http::Method::GET)
                    .uri("/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(result["status"], "running");
        assert_eq!(result["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(result["dropped_events"], 0, "no drops expected in this test");
        assert!(result.get("buffer_occupancy").is_some(), "buffer_occupancy should be present");

        let _ = fs::remove_dir_all(&dir);
    }
}
