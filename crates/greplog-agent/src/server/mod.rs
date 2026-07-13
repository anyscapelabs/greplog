use anyhow::Result;
use axum::{Router, routing::get, Json};
use serde::Serialize;
use std::sync::Arc;
use tracing::info;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: &'static str,
}

/// Embedded health/status HTTP server.
pub struct HttpServer;

impl HttpServer {
    pub fn new() -> Self {
        Self
    }

    pub fn spawn(self, config: Arc<super::Config>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let app = Router::new()
                .route("/health", get(health_handler))
                .route("/status", get(status_handler));

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
        "uptime_secs": 0,
    }))
}
