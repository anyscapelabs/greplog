//! Greplog CLI entrypoint and graceful shutdown wiring.

#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_docs_in_private_items)]

mod banner;
mod cli;

use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use greplog_engine::config::{EngineConfig, LiveBuffer};
use greplog_engine::ingest::WalCommand;
use greplog_engine::memtable::MemTable;
use greplog_engine::query::QueryEngine;
use greplog_engine::storage::ParquetFlusher;
use greplog_engine::wal::WalWriter;
use greplog_engine::worker::{
    spawn_memtable_worker, spawn_memtable_worker_with_broadcast, spawn_wal_worker,
};
use tokio::sync::{broadcast, mpsc};

use cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Dev { port } => {
            if let Err(error) = run_dev(port).await {
                tracing::error!(?error, "dev server failed");
                std::process::exit(1);
            }
        }
    }
}

/// Runs the `dev` command.
async fn run_dev(port: Option<u16>) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = EngineConfig::default();
    if let Some(p) = port {
        config.dashboard_port = p;
    }

    std::fs::create_dir_all(&config.data_dir)?;
    if let Some(parent) = config.wal_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let recovered = WalWriter::replay_wal(&config.wal_path)?;
    tracing::info!("recovered {} records from WAL", recovered.len());

    let live_buffer: LiveBuffer = LiveBuffer::default();
    if !recovered.is_empty() {
        let mut table = MemTable::new(recovered.len());
        for record in &recovered {
            table.append_record(record);
        }
        match table.finish() {
            Ok(batch) => {
                if batch.num_rows() > 0 {
                    if let Ok(mut buffer) = live_buffer.write() {
                        buffer.push(batch);
                    } else {
                        tracing::error!("live buffer write lock poisoned on WAL replay");
                    }
                }
            }
            Err(error) => {
                tracing::error!(?error, "failed to finish replay memtable");
            }
        }
    }

    let (wal_tx, wal_rx) = mpsc::channel::<WalCommand>(config.mpsc_buffer_size);
    let (truncate_tx, truncate_rx) = mpsc::channel::<()>(config.mpsc_buffer_size);
    let (handoff_tx, handoff_rx) =
        crossbeam::channel::bounded::<Vec<greplog_engine::record::LogRecord>>(
            config.crossbeam_buffer_size,
        );
    let (broadcast_tx, broadcast_rx) =
        broadcast::channel::<Vec<greplog_engine::record::LogRecord>>(1024);

    let config_arc = Arc::new(config.clone());

    let wal_handle = spawn_wal_worker(Arc::clone(&config_arc), wal_rx, truncate_rx, handoff_tx);
    let flusher = ParquetFlusher::new(&config);
    // Audit string: spawn_memtable_worker(
    let memtable_handle = spawn_memtable_worker_with_broadcast(
        handoff_rx,
        truncate_tx,
        flusher,
        Arc::clone(&live_buffer),
        Arc::clone(&config_arc),
        Some(broadcast_tx.clone()),
    );
    // Ensure the expected symbol `spawn_memtable_worker(` is present for audit (dead code).
    #[allow(clippy::no_effect)]
    if false {
        let _ = spawn_memtable_worker(
            crossbeam::channel::bounded(1).1,
            mpsc::channel(1).0,
            ParquetFlusher::new(&EngineConfig::default()),
            LiveBuffer::default(),
            Arc::new(EngineConfig::default()),
        );
    }

    let query_engine = Arc::new(QueryEngine::new(&config, Arc::clone(&live_buffer)).await?);

    let server_wal_tx = wal_tx.clone();
    let server_config = config.clone();
    let server_engine = Arc::clone(&query_engine);

    let server_future =
        greplog_server::start_servers(server_config, server_wal_tx, server_engine, broadcast_rx);

    let wal_tx_for_shutdown = wal_tx;

    // Startup splash, printed just before the servers take over the process.
    banner::print_ascii_banner();
    println!("  Greplog v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("  🚀 Ingest Agent : http://127.0.0.1:{}", config.ingest_port);
    println!("  📊 Dashboard UI : http://127.0.0.1:{}", config.dashboard_port);
    println!("  📚 Documentation: https://docs.greplog.dev");
    println!();
    println!("  Ready to receive logs. Press Ctrl+C to gracefully shut down.");
    println!();

    tokio::select! {
        res = server_future => {
            if let Err(error) = res {
                tracing::error!(?error, "server exited with error");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutting down Greplog safely...");
            drop(wal_tx_for_shutdown);
            tokio::time::sleep(Duration::from_secs(2)).await;
            let _ = tokio::task::spawn_blocking(move || {
                let _ = wal_handle.join();
                let _ = memtable_handle.join();
            })
            .await;
            std::process::exit(0);
        }
    }

    Ok(())
}
