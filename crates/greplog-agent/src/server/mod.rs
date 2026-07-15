use super::query::QueryEngine;
use axum::{
    Json,
    extract::State,
    http::{header, Method},
    routing::{get, post},
    Router,
};
use serde::Serialize;
use std::sync::Arc;
use tower::ServiceBuilder;
use tracing::info;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: &'static str,
}

pub struct AppState {
    pub config: Arc<super::Config>,
    pub query_engine: QueryEngine,
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

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn status_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "running",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn query_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<super::query::QueryRequest>,
) -> Result<Json<super::query::QueryResult>, (axum::http::StatusCode, String)> {
    match state.query_engine.query(&req.sql) {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            axum::http::StatusCode::BAD_REQUEST,
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
