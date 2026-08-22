//! Greplog server: dual-port Axum ingest and dashboard.

#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_docs_in_private_items)]

pub mod assets;
pub mod dashboard;
pub mod error;
pub mod ingest;
pub mod shutdown;
pub mod stats;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use greplog_engine::config::EngineConfig;
use greplog_engine::ingest::WalCommand;
use greplog_engine::query::QueryEngine;
use tokio::sync::{broadcast, mpsc};
use tower_http::limit::RequestBodyLimitLayer;

use crate::shutdown::Shutdown;

const MAX_INGEST_BODY_BYTES: usize = 5 * 1024 * 1024;

/// Shared state for the dashboard router: the query engine, the live-tail
/// broadcast sender, the shutdown handle each SSE stream watches, and the
/// data directory backing `/api/stats`.
#[derive(Clone)]
pub(crate) struct DashboardState {
    engine: Arc<QueryEngine>,
    broadcast_tx: broadcast::Sender<Vec<greplog_engine::record::LogRecord>>,
    shutdown: Shutdown,
    pub(crate) data_dir: std::path::PathBuf,
}

impl axum::extract::FromRef<DashboardState> for Arc<QueryEngine> {
    fn from_ref(state: &DashboardState) -> Self {
        Arc::clone(&state.engine)
    }
}

impl axum::extract::FromRef<DashboardState> for dashboard::TailState {
    fn from_ref(state: &DashboardState) -> Self {
        dashboard::TailState {
            records: state.broadcast_tx.subscribe(),
            shutdown: state.shutdown.clone(),
        }
    }
}

/// Starts the ingest and dashboard servers and returns once both have drained.
///
/// Both listeners bind before either is served, so a port conflict is reported
/// instead of leaving one half of the system listening. When `shutdown` fires,
/// both stop accepting and drain in-flight requests; dropping the per-
/// connection tasks closes the last clones of `wal_tx`, which is what lets the
/// WAL worker observe its channel close — this function returning is the
/// caller's signal that it is safe to join the write path.
pub async fn start_servers(
    config: EngineConfig,
    wal_tx: mpsc::Sender<WalCommand>,
    query_engine: Arc<QueryEngine>,
    broadcast_tx: broadcast::Sender<Vec<greplog_engine::record::LogRecord>>,
    shutdown: Shutdown,
) -> Result<(), std::io::Error> {
    let ingest_router = Router::new()
        .route("/api/log", post(ingest::handle_ingest))
        .layer(RequestBodyLimitLayer::new(MAX_INGEST_BODY_BYTES))
        .with_state(wal_tx);

    let dashboard_state = DashboardState {
        engine: query_engine,
        broadcast_tx,
        shutdown: shutdown.clone(),
        data_dir: config.data_dir.clone(),
    };

    let dashboard_router = Router::new()
        .route("/api/query", post(dashboard::handle_query))
        .route("/api/search", post(dashboard::handle_search))
        .route("/api/stats", get(dashboard::handle_stats))
        .route("/api/tail", get(dashboard::handle_tail))
        .fallback(assets::handle_asset)
        .with_state(dashboard_state);

    let ingest_listener =
        tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.ingest_port)).await?;
    let dashboard_listener =
        tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.dashboard_port)).await?;

    let ingest_shutdown = shutdown.clone();
    let ingest_server = axum::serve(ingest_listener, ingest_router)
        .with_graceful_shutdown(async move { ingest_shutdown.wait().await });
    let dashboard_server = axum::serve(dashboard_listener, dashboard_router)
        .with_graceful_shutdown(async move { shutdown.wait().await });

    let (ingest_res, dashboard_res) = tokio::join!(ingest_server, dashboard_server);
    ingest_res?;
    dashboard_res?;
    Ok(())
}
