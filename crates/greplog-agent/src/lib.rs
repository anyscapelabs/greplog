use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

pub mod detect;
mod ingest;
pub mod query;
pub mod query_engine;
pub mod server;
mod store;

pub use store::Writer;

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

    pub fn db_path(&self) -> PathBuf {
        self.workspace.join(".greplog/store.db")
    }
}

pub async fn run(config: Config) -> Result<()> {
    let config = Arc::new(config);

    tokio::fs::create_dir_all(config.socket_dir()).await?;

    let (batch_tx, batch_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1024);

    let writer = store::Writer::new(config.db_path())?;
    let writer_handle = writer.spawn(batch_rx, config.flush_interval_secs);

    let ingest_handle = ingest::IngestServer::new(batch_tx).spawn(&config);

    let query_engine = query::QueryEngine::new(config.db_path())?;

    let server_state = server::AppState {
        config: config.clone(),
        query_engine,
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
