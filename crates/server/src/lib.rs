//! Greplog server: dual-port Axum ingest and dashboard.

#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_docs_in_private_items)]

pub mod dashboard;
pub mod error;
pub mod ingest;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use greplog_engine::config::EngineConfig;
use greplog_engine::ingest::WalCommand;
use greplog_engine::query::QueryEngine;
use tokio::sync::{broadcast, mpsc};
use tower_http::limit::RequestBodyLimitLayer;

/// Documented ceiling for a single ingest batch, in bytes.
const MAX_INGEST_BODY_BYTES: usize = 5 * 1024 * 1024;

/// Shared state for the dashboard router.
///
/// Holds the query engine and a broadcast sender that fans out live logs to
/// SSE subscribers. The sender is `Clone`, so the whole state is `Clone` as
/// required by `axum::Router`.
#[derive(Clone)]
struct DashboardState {
    engine: Arc<QueryEngine>,
    broadcast_tx: broadcast::Sender<Vec<greplog_engine::record::LogRecord>>,
}

impl axum::extract::FromRef<DashboardState> for Arc<QueryEngine> {
    fn from_ref(state: &DashboardState) -> Self {
        Arc::clone(&state.engine)
    }
}

impl axum::extract::FromRef<DashboardState> for broadcast::Receiver<Vec<greplog_engine::record::LogRecord>> {
    fn from_ref(state: &DashboardState) -> Self {
        state.broadcast_tx.subscribe()
    }
}

/// Starts the ingest and dashboard servers concurrently.
///
/// The ingest API listens on `config.ingest_port` and exposes `POST /api/log`.
/// The dashboard API listens on `config.dashboard_port` and exposes
/// `POST /api/query` and `GET /api/tail`.
///
/// The two servers are run concurrently via `tokio::join!`.
///
/// # Errors
///
/// Returns an `std::io::Error` if either `TcpListener` fails to bind.
pub async fn start_servers(
    config: EngineConfig,
    wal_tx: mpsc::Sender<WalCommand>,
    query_engine: Arc<QueryEngine>,
    broadcast_rx: broadcast::Receiver<Vec<greplog_engine::record::LogRecord>>,
) -> Result<(), std::io::Error> {
    // Bridge the incoming `broadcast_rx` (which has no cloneable sender) into
    // a new broadcast channel that can be cloned per SSE connection. A
    // background forwarder pumps messages from the original receiver into the
    // new sender.
    let (broadcast_tx, _) = broadcast::channel::<Vec<greplog_engine::record::LogRecord>>(1024);
    let forward_tx = broadcast_tx.clone();
    let mut rx = broadcast_rx;
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let _ = forward_tx.send(msg);
        }
    });

    let ingest_router = Router::new()
        .route("/api/log", post(ingest::handle_ingest))
        .layer(RequestBodyLimitLayer::new(MAX_INGEST_BODY_BYTES))
        .with_state(wal_tx);

    let dashboard_state = DashboardState {
        engine: query_engine,
        broadcast_tx,
    };

    let dashboard_router = Router::new()
        .route("/api/query", post(dashboard::handle_query))
        .route("/api/tail", get(dashboard::handle_tail))
        .with_state(dashboard_state);

    let ingest_addr = format!("0.0.0.0:{}", config.ingest_port);
    let dashboard_addr = format!("0.0.0.0:{}", config.dashboard_port);

    let ingest_listener = tokio::net::TcpListener::bind(ingest_addr).await?;
    let dashboard_listener = tokio::net::TcpListener::bind(dashboard_addr).await?;

    let ingest_server = axum::serve(ingest_listener, ingest_router);
    let dashboard_server = axum::serve(dashboard_listener, dashboard_router);

    // Run both servers concurrently.
    let (ingest_res, dashboard_res) = tokio::join!(ingest_server, dashboard_server);
    ingest_res?;
    dashboard_res?;
    Ok(())
}
