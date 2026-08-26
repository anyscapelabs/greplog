//! Greplog CLI entrypoint and graceful shutdown wiring.

#![warn(clippy::all, clippy::pedantic)]
// Pedantic style calls: error docs per function are noise here; the types
// and the central error enum carry that contract.
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
#![allow(clippy::missing_docs_in_private_items)]

mod banner;
mod cli;
mod ui;
mod uninstall;

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread::JoinHandle;
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
use greplog_engine::worker::{spawn_memtable_worker_with_broadcast, spawn_wal_worker};
use greplog_server::shutdown::{self, ShutdownTrigger};
use tokio::sync::{broadcast, mpsc};

use cli::{Cli, Commands};

const TAIL_BROADCAST_CAPACITY: usize = 1_024;

const DRAIN_TIMEOUT: Duration = Duration::from_secs(20);

const EXIT_DRAIN_TIMEOUT: i32 = 75;

/// Exit code when a second `Ctrl+C` forces an immediate stop.
const EXIT_INTERRUPTED: i32 = 130;

#[tokio::main]
async fn main() {
    // Compact, target-less lines: the operator cares about what happened,
    // not which module said it.
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Dev { host, port } => {
            if let Err(error) = run_dev(host, port).await {
                tracing::error!(?error, "dev server failed");
                std::process::exit(1);
            }
        }
        Commands::Start {
            host,
            port,
            retention_days,
        } => {
            if let Err(error) = run_start(host, port, retention_days).await {
                tracing::error!(?error, "server failed");
                std::process::exit(1);
            }
        }
        Commands::Status => run_status(),
        Commands::Uninstall { yes } => {
            if let Err(error) = uninstall::run(yes) {
                tracing::error!(?error, "uninstall failed");
                std::process::exit(1);
            }
        }
    }
}

async fn run_dev(
    host: Option<String>,
    port: Option<u16>,
) -> Result<(), Box<dyn std::error::Error>> {
    let defaults = EngineConfig::default();
    let config = EngineConfig {
        bind_host: host.unwrap_or_else(|| defaults.bind_host.clone()),
        dashboard_port: port.unwrap_or(defaults.dashboard_port),
        ..defaults
    };
    run_server(config).await
}

async fn run_start(
    host: Option<String>,
    port: Option<u16>,
    retention_days: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let defaults = EngineConfig::default();
    let config = EngineConfig {
        bind_host: host.unwrap_or_else(|| defaults.bind_host.clone()),
        dashboard_port: port.unwrap_or(defaults.dashboard_port),
        retention_days: retention_days.or(defaults.retention_days),
        ..defaults
    };
    run_server(config).await
}

fn run_status() {
    use ui::{bold, dim, storage_bytes};

    let config = EngineConfig::default();

    let sealed =
        greplog_engine::wal::sealed_segments(&config.wal_path).map_or(0, |segments| segments.len());
    let storage = greplog_server::stats::storage_stats(&config.data_dir);
    let services = service_breakdown(&config.data_dir);

    println!("{}", bold("Greplog status"));
    let row = |label: &str, value: String| println!("  {:<12} {}", dim(label), value);
    row("data dir", config.data_dir.display().to_string());
    row(
        "wal",
        format!(
            "{} · {sealed} sealed segment{} · {}",
            config.wal_path.display(),
            if sealed == 1 { "" } else { "s" },
            storage_bytes(wal_usage(&config.wal_path))
        ),
    );
    match config.retention_days {
        Some(days) => row("retention", format!("{days} days")),
        None => row("retention", "disabled".to_string()),
    }
    row(
        "storage",
        format!(
            "{} partition{} · {} parquet file{} · {}",
            storage.partitions,
            if storage.partitions == 1 { "" } else { "s" },
            storage.chunks,
            if storage.chunks == 1 { "" } else { "s" },
            storage_bytes(storage.bytes),
        ),
    );
    if services.is_empty() {
        row("services", dim("(no chunks on disk yet)"));
    } else {
        let rendered: Vec<String> = services
            .iter()
            .map(|(name, rows)| format!("{name} ({})", bold(&rows.to_string())))
            .collect();
        row("services", rendered.join(&dim(" · ")));
    }
}

/// Per-service stored row counts by walking `service=` directories; a chunk
/// whose footer cannot be read contributes its files but zero rows.
fn service_breakdown(data_dir: &Path) -> Vec<(String, u64)> {
    let mut totals: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    collect_service_rows(data_dir, &mut totals);
    let mut merged: Vec<(String, u64)> = totals.into_iter().collect();
    merged.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    merged
}

fn collect_service_rows(dir: &Path, totals: &mut std::collections::HashMap<String, u64>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Some(service) = name.strip_prefix("service=") {
            *totals.entry(service.to_string()).or_insert(0) += walk_chunk_rows(&path);
        } else {
            collect_service_rows(&path, totals);
        }
    }
}

fn walk_chunk_rows(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "parquet"))
        .map(|entry| greplog_engine::storage::chunk_row_count(&entry.path()).unwrap_or(0))
        .sum()
}

/// Total WAL bytes on disk: the active segment plus every sealed one. Only
/// reporting `current.wal` understates the footprint exactly when an operator
/// runs `status` to find out where the disk went.
fn wal_usage(wal_path: &Path) -> u64 {
    let mut bytes = fs::metadata(wal_path).map_or(0, |meta| meta.len());
    for segment in greplog_engine::wal::sealed_segments(wal_path).unwrap_or_default() {
        bytes += fs::metadata(&segment).map_or(0, |meta| meta.len());
    }
    bytes
}

async fn run_server(config: EngineConfig) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&config.data_dir)?;
    if let Some(parent) = config.wal_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let live_buffer: LiveBuffer = LiveBuffer::default();
    recover_wal_into(&config, &live_buffer)?;

    let (wal_tx, wal_rx) = mpsc::channel::<WalCommand>(config.mpsc_buffer_size);
    let (truncate_tx, truncate_rx) = mpsc::channel::<usize>(config.mpsc_buffer_size);
    let (handoff_tx, handoff_rx) = crossbeam::channel::bounded::<
        Vec<greplog_engine::record::LogRecord>,
    >(config.crossbeam_buffer_size);
    let (broadcast_tx, _) =
        broadcast::channel::<Vec<greplog_engine::record::LogRecord>>(TAIL_BROADCAST_CAPACITY);

    let config_arc = Arc::new(config.clone());

    let wal_handle = spawn_wal_worker(Arc::clone(&config_arc), wal_rx, truncate_rx, handoff_tx);
    let memtable_handle = spawn_memtable_worker_with_broadcast(
        handoff_rx,
        truncate_tx,
        ParquetFlusher::new(&config),
        Arc::clone(&live_buffer),
        Arc::clone(&config_arc),
        Some(broadcast_tx.clone()),
    );

    let query_engine = Arc::new(QueryEngine::new(&config, Arc::clone(&live_buffer)).await?);

    let (trigger, shutdown) = shutdown::channel();

    // Aborted at shutdown; a merge already in flight finishes on the blocking
    // pool and either lands atomically or leaves a `.part` the next run discards.
    let compactor_task = tokio::spawn(Compactor::start_background_loop(config.clone()));

    let retention_task = tokio::spawn(Retention::start_background_loop(config.clone()));

    spawn_signal_listener(trigger);

    print_startup_banner(&config);

    let server_result =
        greplog_server::start_servers(config, wal_tx.clone(), query_engine, broadcast_tx, shutdown)
            .await;

    if let Err(error) = &server_result {
        tracing::error!(?error, "server exited with error");
    }

    // Servers drained: dropping this last ingest-sender clone closes the WAL
    // worker's channel, which closes the handoff channel and lets the
    // MemTable worker flush its remainder and exit.
    drop(wal_tx);
    compactor_task.abort();
    retention_task.abort();

    drain_write_path(wal_handle, memtable_handle).await;
    println!(
        "  {} write path drained — every acknowledged row is on disk",
        ui::ok_mark()
    );

    server_result.map_err(Into::into)
}

fn recover_wal_into(
    config: &EngineConfig,
    live_buffer: &LiveBuffer,
) -> Result<(), Box<dyn std::error::Error>> {
    let recovered = replay_all(&config.wal_path)?;
    tracing::info!(
        "recovered {} records from WAL (sealed segments + active)",
        recovered.len()
    );
    if recovered.is_empty() {
        return Ok(());
    }

    let mut table = MemTable::new(recovered.len());
    for record in &recovered {
        table.append_record(record);
    }
    let batch = match table.finish() {
        Ok(batch) => batch,
        Err(error) => {
            tracing::error!(?error, "failed to finish replay memtable");
            return Ok(());
        }
    };
    if batch.num_rows() == 0 {
        return Ok(());
    }
    if let Ok(mut buffer) = live_buffer.write() {
        buffer.push(batch);
    } else {
        tracing::error!("live buffer write lock poisoned on WAL replay");
    }
    Ok(())
}

fn spawn_signal_listener(trigger: ShutdownTrigger) {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_err() {
            tracing::error!("failed to listen for Ctrl+C; shutdown must be signalled externally");
            return;
        }
        println!();
        tracing::info!("shutting down Greplog safely (press Ctrl+C again to force)");
        trigger.fire();

        if tokio::signal::ctrl_c().await.is_ok() {
            tracing::warn!("second Ctrl+C received; exiting without draining the write path");
            std::process::exit(EXIT_INTERRUPTED);
        }
    });
}

async fn drain_write_path(wal_handle: JoinHandle<()>, memtable_handle: JoinHandle<()>) {
    let joined = tokio::task::spawn_blocking(move || {
        if wal_handle.join().is_err() {
            tracing::error!("wal worker panicked");
        }
        if memtable_handle.join().is_err() {
            tracing::error!("memtable worker panicked");
        }
    });

    match tokio::time::timeout(DRAIN_TIMEOUT, joined).await {
        Ok(Ok(())) => tracing::info!("write path drained; all buffered rows are on disk"),
        Ok(Err(error)) => tracing::error!(?error, "drain task failed to run"),
        Err(_) => {
            tracing::error!(
                timeout_secs = DRAIN_TIMEOUT.as_secs(),
                "write path did not drain in time; exiting with buffered rows still in the WAL"
            );
            std::process::exit(EXIT_DRAIN_TIMEOUT);
        }
    }
}

/// The address an operator can actually type: a wildcard bind serves every
/// interface including loopback, so `127.0.0.1` is always valid there.
fn display_host(bind_host: &str) -> &str {
    match bind_host {
        "" | "0.0.0.0" | "::" => "127.0.0.1",
        host => host,
    }
}

fn print_startup_banner(config: &EngineConfig) {
    use ui::{bold, dim, link, wrap};

    let art = banner::art_lines();
    let info = {
        let tagline = "Fast, lightweight, zero-data-loss logging engine and dashboard \
                       for solo developers, startups, and small teams.";
        let mut lines: Vec<String> = wrap(tagline, 58)
            .into_iter()
            .map(|line| dim(&line))
            .collect();
        let host = display_host(&config.bind_host);
        lines.push(String::new());
        lines.push(format!(
            "{:<10} {}",
            dim("Dashboard"),
            link(&format!("http://{host}:{}", config.dashboard_port))
        ));
        lines.push(format!(
            "{:<10} {}",
            dim("Ingest"),
            link(&format!(
                "POST http://{host}:{}/api/log",
                config.ingest_port
            ))
        ));
        match config.retention_days {
            Some(days) => lines.push(format!(
                "{:<10} {} · {} retention",
                dim("Storage"),
                config.data_dir.display(),
                bold(&format!("{days} day"))
            )),
            None => lines.push(format!(
                "{:<10} {} · retention disabled",
                dim("Storage"),
                config.data_dir.display()
            )),
        }
        lines.push(format!(
            "{:<10} {}",
            dim("Docs"),
            link("https://docs.greplog.dev")
        ));
        lines.push(String::new());
        lines.push(format!(
            "  {} Ready. Ctrl+C to drain and exit.",
            ui::ok_mark()
        ));
        lines
    };

    print!("{}", compose_screen(&art, &info));
}

/// Lays out art and info side by side on wide terminals, stacked otherwise.
fn compose_screen(art: &[String], info: &[String]) -> String {
    let term_width =
        crossterm::terminal::window_size().map_or(80, |size| usize::from(size.columns));
    compose_screen_for(art, info, term_width)
}

fn compose_screen_for(art: &[String], info: &[String], term_width: usize) -> String {
    let mut out = String::from("\n");

    // Side-by-side only when both columns plus a gutter actually fit.
    if term_width >= banner::ART_WIDTH + 62 {
        let offset = art.len().saturating_sub(info.len()) / 2;
        let blank_art = " ".repeat(banner::ART_WIDTH + 2);
        for index in 0..art.len().max(info.len() + offset) {
            let left = art.get(index).map_or(blank_art.as_str(), String::as_str);
            let info_line = if index >= offset {
                info.get(index - offset)
            } else {
                None
            };
            match info_line {
                Some(line) if !line.is_empty() => {
                    out.push_str(left);
                    out.push_str("  ");
                    out.push_str(line);
                    out.push('\n');
                }
                _ => {
                    out.push_str(left);
                    out.push('\n');
                }
            }
        }
    } else {
        for line in art {
            out.push_str(line);
            out.push('\n');
        }
        for line in info {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push('\n');
    out
}
#[cfg(test)]
mod screen_tests {
    #[test]
    fn wide_terminals_put_info_beside_the_logo() {
        // Real banner lines are uniformly ART_WIDTH wide.
        let art: Vec<String> = (0..18).map(|row| format!("art{row:<43}")).collect();
        let info: Vec<String> = (0..8).map(|row| format!("info{row}")).collect();
        let screen = super::compose_screen_for(&art, &info, 140);

        let offset = (art.len() - info.len()) / 2;
        let expected_prefix = super::banner::ART_WIDTH + 2;
        let info_row = screen
            .lines()
            .enumerate()
            .find(|(index, line)| *index >= offset && line.trim_end().ends_with("info0"))
            .map(|(_, line)| line)
            .expect("info column rendered beside the logo");
        assert_eq!(
            info_row.find("info0"),
            Some(expected_prefix),
            "info starts right of the logo with a two-space gutter"
        );
    }

    #[test]
    fn narrow_terminals_stack_the_columns() {
        let art: Vec<String> = (0..4).map(|row| format!("art{row}")).collect();
        let info: Vec<String> = vec!["info0".to_string()];
        let screen = super::compose_screen_for(&art, &info, 70);
        assert!(screen.lines().any(|line| line == "art0"), "stacked art");
        assert!(screen.lines().any(|line| line == "  info0"), "stacked info");
    }
}
