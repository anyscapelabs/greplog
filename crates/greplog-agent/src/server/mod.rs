use super::query_engine::QueryEngine;
use axum::{
    extract::State,
    http::{header, Method, StatusCode},
    response::sse::{Event, KeepAlive},
    response::Sse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
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
    pub live_tx: Option<broadcast::Sender<String>>,
}

/// JSON request body for `POST /query`.
#[derive(Debug, Clone, Deserialize)]
pub struct QueryRequest {
    pub sql: String,
}

pub struct HttpServer;

impl Default for HttpServer {
    fn default() -> Self {
        Self::new()
    }
}

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
                .route("/tail", get(tail_handler))
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

async fn status_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
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
        Err(e) => Err((StatusCode::BAD_REQUEST, format!("Query failed: {e}"))),
    }
}

async fn detect_handler(State(state): State<Arc<AppState>>) -> Json<Vec<super::detect::Detection>> {
    let detections = super::detect::detect_workspace(&state.config.workspace);
    Json(detections.unwrap_or_default())
}

async fn resources_handler(
    State(state): State<Arc<AppState>>,
) -> Json<super::detect::system::SystemResources> {
    let resources = super::detect::system::collect_system_resources(&state.config.workspace);
    Json(resources)
}

pub(crate) async fn tail_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state
        .live_tx
        .as_ref()
        .map(|tx| tx.subscribe())
        .unwrap_or_else(|| {
            let (tx, rx) = broadcast::channel(1);
            drop(tx);
            rx
        });

    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(data) => Some(Ok(Event::default().data(data))),
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(
        KeepAlive::default()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "../../tests/modules/server/mod.rs"]
mod tests;
