use anyhow::Result;
use greplog_core::gen::IngestBatch;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tracing::info;

pub mod detect;
mod ingest;
pub mod query_engine;
pub mod server;
mod store;

/// Maximum in‑memory events before the buffer rejects new batches.
const MAX_BUFFER_EVENTS: usize = 50_000;

/// Channel capacity between the ingest handler and the writer.
const INGEST_CHANNEL_CAPACITY: usize = 1024;

#[derive(Debug, Clone, clap::Parser)]
pub struct Config {
    #[arg(long, default_value = ".")]
    pub workspace: PathBuf,

    #[arg(long, default_value_t = 4318)]
    pub tcp_port: u16,

    #[arg(long, default_value_t = 4317)]
    pub ui_port: u16,

    #[arg(long, default_value_t = 2)]
    pub flush_interval_secs: u64,

    #[arg(long, default_value = ".greplog/greplog.sock")]
    pub socket_path: PathBuf,
}

impl Config {
    pub fn socket_dir(&self) -> PathBuf {
        self.workspace.join(".greplog")
    }

    pub fn abs_socket_path(&self) -> PathBuf {
        self.workspace.join(&self.socket_path)
    }
}

pub async fn run(config: Config) -> Result<()> {
    let config = Arc::new(config);

    tokio::fs::create_dir_all(config.socket_dir()).await?;

    let (batch_tx, batch_rx) =
        mpsc::channel::<(IngestBatch, oneshot::Sender<greplog_core::gen::IngestResponse>)>(
            INGEST_CHANNEL_CAPACITY,
        );

    let writer = store::Writer::new(&config.workspace, MAX_BUFFER_EVENTS)?;
    // Run the full startup recovery sequence before accepting any connections:
    // cleanup, compaction reconciliation, WAL replay with dedup, and immediate
    // flush.  This guarantees that ingest listeners only see a fully‑recovered
    // data plane.
    writer.run_startup(50_000)?;
    let dropped_events = writer.dropped_events();
    let writer_handle = writer.spawn(
        batch_rx,
        Duration::from_secs(config.flush_interval_secs),
    );

    let ingest_handle = ingest::IngestServer::new(batch_tx).spawn(&config);

    let query_engine = query_engine::QueryEngine::new(&config.workspace)?;

    let server_state = server::AppState {
        config: config.clone(),
        query_engine,
        dropped_events,
    };
    let server_handle = server::HttpServer::new().spawn(server_state);

    let socket_path = config.abs_socket_path();

    tokio::select! {
        _ = writer_handle => {
            info!("Writer exited");
        },
        _ = ingest_handle => {
            info!("Ingest exited");
        },
        _ = server_handle => {
            info!("HTTP server exited");
        },
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down gracefully");
        },
    }

    if socket_path.exists() {
        if let Err(e) = tokio::fs::remove_file(&socket_path).await {
            tracing::warn!("Failed to clean up socket {:?}: {}", socket_path, e);
        } else {
            info!("Cleaned up socket {:?}", socket_path);
        }
    }

    Ok(())
}
