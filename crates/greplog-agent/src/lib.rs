use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

mod ingest;
mod server;
mod store;

pub use store::Writer;

/// Agent configuration.
#[derive(Debug, Clone, clap::Parser)]
pub struct Config {
    /// Workspace root directory (defaults to cwd).
    #[arg(long, default_value = ".")]
    pub workspace: PathBuf,

    /// TCP fallback port.
    #[arg(long, default_value_t = 4318)]
    pub tcp_port: u16,

    /// Dashboard UI port.
    #[arg(long, default_value_t = 4317)]
    pub ui_port: u16,

    /// Flush interval in seconds.
    #[arg(long, default_value_t = 2)]
    pub flush_interval_secs: u64,

    /// UDS socket path (relative to workspace).
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

/// Start the agent: ingest → store → server.
pub async fn run(config: Config) -> Result<()> {
    let config = Arc::new(config);

    // Create workspace directories
    tokio::fs::create_dir_all(config.socket_dir()).await?;

    // Channel: ingest workers → single writer
    let (batch_tx, batch_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1024);

    // Store writer
    let writer = store::Writer::new(config.db_path())?;
    let writer_handle = writer.spawn(batch_rx, config.flush_interval_secs);

    // Ingest listeners
    let ingest_handle = ingest::IngestServer::new(batch_tx).spawn(&config);

    // Health / status HTTP
    let server_handle = server::HttpServer::new().spawn(config.clone());

    // Wait for any to exit (they run until Ctrl+C)
    tokio::select! {
        _ = writer_handle => {},
        _ = ingest_handle => {},
        _ = server_handle => {},
    }

    Ok(())
}
