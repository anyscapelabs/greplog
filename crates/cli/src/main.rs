//! Greplog CLI entrypoint and graceful shutdown wiring.

#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_docs_in_private_items)]

mod banner;
mod cli;

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use greplog_engine::compactor::Compactor;
use greplog_engine::config::{EngineConfig, LiveBuffer};
use greplog_engine::ingest::WalCommand;
use greplog_engine::memtable::MemTable;
use greplog_engine::query::QueryEngine;
use greplog_engine::retention::Retention;
use greplog_engine::storage::ParquetFlusher;
use greplog_engine::wal::replay_all;
use greplog_engine::worker::{
    spawn_memtable_worker_with_broadcast, spawn_wal_worker,
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
        Commands::Start { port, retention_days } => {
            if let Err(error) = run_start(port, retention_days).await {
                tracing::error!(?error, "server failed");
                std::process::exit(1);
            }
        }
        Commands::Status => {
            if let Err(error) = run_status() {
                eprintln!("status failed: {error}");
                std::process::exit(1);
            }
        }
    }
}

/// Runs the `dev` command: a server with engine defaults.
async fn run_dev(port: Option<u16>) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = EngineConfig::default();
    if let Some(p) = port {
        config.dashboard_port = p;
    }
    run_server(config).await
}

/// Runs the `start` command: a server with retention configurable.
async fn run_start(
    port: Option<u16>,
    retention_days: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = EngineConfig::default();
    if let Some(p) = port {
        config.dashboard_port = p;
    }
    if let Some(days) = retention_days {
        config.retention_days = Some(days);
    }
    run_server(config).await
}

/// Reports WAL and storage status directly from the filesystem, so it works
/// without a running server.
fn run_status() -> Result<(), Box<dyn std::error::Error>> {
    let config = EngineConfig::default();

    let wal_bytes = fs::metadata(&config.wal_path).map(|meta| meta.len()).unwrap_or(0);
    let (partitions, chunks, bytes) = storage_usage(&config.data_dir);

    println!("Greplog status");
    println!("  data dir   : {}", config.data_dir.display());
    println!("  wal        : {} ({} bytes)", config.wal_path.display(), wal_bytes);
    match config.retention_days {
        Some(days) => println!("  retention  : {days} days"),
        None => println!("  retention  : disabled"),
    }
    println!("  partitions : {partitions}");
    println!("  parquet files : {chunks}");
    println!("  disk usage : {} bytes", bytes);
    Ok(())
}

/// Counts day partitions, parquet chunks, and total disk bytes under `data_dir`.
fn storage_usage(data_dir: &Path) -> (usize, usize, u64) {
    let mut partitions = 0usize;
    let mut chunks = 0usize;
    let mut bytes = 0u64;
    walk_usage(data_dir, &mut partitions, &mut chunks, &mut bytes);
    (partitions, chunks, bytes)
}

/// Recursively accumulates storage statistics, tolerating a missing root.
fn walk_usage(
    dir: &Path,
    partitions: &mut usize,
    chunks: &mut usize,
    bytes: &mut u64,
) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "day=" || name.to_string_lossy().starts_with("day=")) {
                *partitions += 1;
            }
            walk_usage(&path, partitions, chunks, bytes);
        } else if path.extension().is_some_and(|ext| ext == "parquet") {
            *chunks += 1;
            if let Ok(meta) = fs::metadata(&path) {
                *bytes += meta.len();
            }
        }
    }
}

/// Runs the shared server body: WAL replay, workers, compactor, retention, and
/// the dual-port servers.
async fn run_server(config: EngineConfig) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&config.data_dir)?;
    if let Some(parent) = config.wal_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let recovered = replay_all(&config.wal_path)?;
    tracing::info!(
        "recovered {} records from WAL (sealed segments + active)",
        recovered.len()
    );

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
    let (truncate_tx, truncate_rx) = mpsc::channel::<usize>(config.mpsc_buffer_size);
    let (handoff_tx, handoff_rx) =
        crossbeam::channel::bounded::<Vec<greplog_engine::record::LogRecord>>(
            config.crossbeam_buffer_size,
        );
    let (broadcast_tx, broadcast_rx) =
        broadcast::channel::<Vec<greplog_engine::record::LogRecord>>(1024);

    let config_arc = Arc::new(config.clone());

    let wal_handle = spawn_wal_worker(Arc::clone(&config_arc), wal_rx, truncate_rx, handoff_tx);
    let flusher = ParquetFlusher::new(&config);
    let memtable_handle = spawn_memtable_worker_with_broadcast(
        handoff_rx,
        truncate_tx,
        flusher,
        Arc::clone(&live_buffer),
        Arc::clone(&config_arc),
        Some(broadcast_tx.clone()),
    );

    let query_engine = Arc::new(QueryEngine::new(&config, Arc::clone(&live_buffer)).await?);

    // Background compactor: sweeps the partition tree every
    // `compaction_run_interval_secs`, merging crowded partitions into single
    // highly compressed chunks.
    let compactor_config = config.clone();
    let _compactor_handle = tokio::spawn(async move {
        Compactor::start_background_loop(compactor_config).await;
    });

    // Background retention: every `retention_run_interval_secs`, purges day
    // partitions older than `retention_days`.
    let retention_config = config.clone();
    let _retention_handle = tokio::spawn(async move {
        Retention::start_background_loop(retention_config).await;
    });

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
