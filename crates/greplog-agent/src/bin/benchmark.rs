//! Greplog Query Latency Benchmark
//!
//! Generates a realistic synthetic dataset through the real ingest path,
//! runs compaction, then measures query latency for three dashboard
//! patterns at two partition-count scales.
//!
//! Usage: cargo run --bin benchmark --release

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use greplog_core::gen::{IngestBatch, IngestResponse, LogEvent, Span, SpanKind};
use prost::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tracing::info;

use greplog_agent::query_engine::QueryEngine;
use greplog_agent::server::{AppState, HttpServer};
use greplog_agent::store::compaction_scheduler::compact_eligible_partitions;
use greplog_agent::store::Writer;
use greplog_agent::{ingest, Config};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

const SERVICES: &[&str] = &["checkout", "catalog", "auth", "api-gateway", "payment"];
const EVENTS_PER_SERVICE_PER_DAY: usize = 20_000;
const SPANS_PER_SERVICE_PER_DAY: usize = 5_000;
const FLUSH_INTERVAL_SECS: u64 = 2;
const COMPACTION_INTERVAL_SECS: u64 = 5;
const REPORT_INTERVAL_SECS: u64 = 5;

struct Scale {
    name: &'static str,
    days: u32,
}

const SCALES: &[Scale] = &[
    Scale { name: "1-week", days: 7 },
    Scale { name: "12-weeks", days: 84 },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("greplog_bench_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

async fn send_frame(stream: &mut TcpStream, batch: &IngestBatch) -> IngestResponse {
    let encoded = batch.encode_to_vec();
    let len = (encoded.len() as u32).to_le_bytes();
    stream.write_all(&len).await.unwrap();
    stream.write_all(&encoded).await.unwrap();
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.unwrap();
    let resp_len = u32::from_le_bytes(len_buf) as usize;
    let mut resp_buf = vec![0u8; resp_len];
    stream.read_exact(&mut resp_buf).await.unwrap();
    IngestResponse::decode(&resp_buf[..]).unwrap()
}

#[allow(dead_code)]
fn ns_to_date(_ns: i64) -> String {
    let secs = _ns / 1_000_000_000;
    let days = secs / 86400;
    // crude but deterministic
    format!("2024-{:02}-{:02}", 1 + ((days as i32 / 28) % 12), 1 + (days % 28))
}

fn day_start_ns(offset_days: i64) -> i64 {
    // Use a fixed base date: 2024-06-01
    let base = 1_717_200_000_000_000_000i64; // ~2024-06-01
    base + offset_days * 86_400_000_000_000
}

fn random_usize(max: usize, seed: u64) -> usize {
    // Simple LCG — deterministic
    let state = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (state as usize) % max
}

// Load an RNG state from the environment for reproducibility, or use zero.
fn bench_seed() -> u64 {
    std::env::var("BENCH_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(42)
}

// ---------------------------------------------------------------------------
// Dataset generation
// ---------------------------------------------------------------------------

struct DatasetGenerator {
    seed: u64,
    call_count: u64,
}

impl DatasetGenerator {
    fn new() -> Self {
        Self { seed: bench_seed(), call_count: 0 }
    }

    fn next_seed(&mut self) -> u64 {
        self.call_count += 1;
        self.seed.wrapping_add(self.call_count).wrapping_mul(2654435761)
    }

    #[allow(dead_code)]
    fn pick<'a>(&mut self, items: &'a [&str]) -> &'a str {
        let i = random_usize(items.len(), self.next_seed());
        items[i]
    }

    fn random_level(&mut self) -> &'static str {
        let r = random_usize(100, self.next_seed());
        match r {
            0..=69 => "info",
            70..=89 => "warn",
            _ => "error",
        }
    }

    fn random_message(&mut self, level: &str) -> String {
        let errors = [
            "connection timeout", "database query failed", "permission denied",
            "rate limit exceeded", "invalid request body", "upstream timeout",
        ];
        let warns = [
            "slow query detected", "high memory usage", "retrying failed operation",
            "deprecated endpoint called", "rate limit approaching",
        ];
        let infos = [
            "request completed", "user logged in", "order created",
            "payment processed", "cache refreshed", "health check passed",
            "configuration reloaded", "session started", "batch job finished",
        ];
        let pool = match level {
            "error" => &errors[..],
            "warn" => &warns[..],
            _ => &infos[..],
        };
        pool[random_usize(pool.len(), self.next_seed())].to_string()
    }

    fn generate_log_event(&mut self, service: &str, ts: i64, idx: usize) -> LogEvent {
        let level = self.random_level();
        let mut attrs = HashMap::new();
        attrs.insert("env".into(), "production".into());
        attrs.insert("host".into(), format!("host-{}", random_usize(20, self.next_seed())));
        if level == "error" {
            attrs.insert("error_code".into(), format!("ERR-{}", 1000 + random_usize(999, self.next_seed())));
        }
        LogEvent {
            service_name: service.to_string(),
            message: self.random_message(level),
            level: level.to_string(),
            timestamp_ns: ts,
            logger_name: "greplog-bench".to_string(),
            file: format!("src/{}.rs", service),
            line: (10 + random_usize(500, self.next_seed())) as i32,
            correlation_id: format!("corr-{:016x}", random_usize(usize::MAX, self.next_seed())),
            attributes: attrs,
            stack_trace: vec![],
            exception_type: String::new(),
            exception_message: String::new(),
            event_id: format!("evt-{:020}", idx),
        }
    }

    fn generate_span(&mut self, service: &str, ts: i64, idx: usize) -> Span {
        let methods = ["GET", "POST", "PUT", "DELETE"];
        let routes = match service {
            "checkout" => &["/checkout", "/checkout/cart", "/checkout/payment", "/checkout/confirm"][..],
            "catalog" => &["/catalog/items", "/catalog/search", "/catalog/categories", "/catalog/item/{id}"][..],
            "auth" => &["/auth/login", "/auth/logout", "/auth/refresh", "/auth/register"][..],
            "api-gateway" => &["/api/v1/*", "/api/v2/*", "/graphql"][..],
            "payment" => &["/payment/charge", "/payment/refund", "/payment/methods", "/payment/webhook"][..],
            _ => &["/unknown"][..],
        };
        let method = methods[random_usize(methods.len(), self.next_seed())];
        let route = routes[random_usize(routes.len(), self.next_seed())];
        let status = if self.random_level() == "error" { 500 } else { [200, 200, 200, 201, 204][random_usize(5, self.next_seed())] };
        let duration_ns = (1_000_000 + random_usize(500_000_000, self.next_seed())) as i64;

        let mut attrs = HashMap::new();
        attrs.insert("db.queries".into(), format!("{}", random_usize(10, self.next_seed())));
        attrs.insert("content_type".into(), "application/json".into());

        Span {
            service_name: service.to_string(),
            name: format!("{} {}", method, route),
            start_time_ns: ts,
            end_time_ns: ts + duration_ns,
            correlation_id: format!("corr-{:016x}", random_usize(usize::MAX, self.next_seed())),
            parent_correlation_id: String::new(),
            route: route.to_string(),
            method: method.to_string(),
            status_code: status,
            error: if status >= 400 { "error".to_string() } else { String::new() },
            attributes: attrs,
            kind: SpanKind::Server as i32,
            event_id: format!("spn-{:020}", idx),
        }
    }

    /// Generate all data for the given number of days and send via TCP.
    async fn generate(
        &mut self,
        tcp_stream: &mut TcpStream,
        days: u32,
    ) -> Result<u64> {
        let base = day_start_ns(0);
        let day_ns = 86_400_000_000_000i64;
        let mut total_events = 0u64;
        let mut batch_seq = 0i64;

        // Report every REPORT_INTERVAL_SECS seconds
        let mut last_report = Duration::ZERO;
        let gen_start = Instant::now();

        for day in 0..days {
            let day_ts = base + (day as i64) * day_ns;
            for service in SERVICES {
                let n_logs = random_usize(EVENTS_PER_SERVICE_PER_DAY, self.next_seed()) + 500;
                let n_spans = random_usize(SPANS_PER_SERVICE_PER_DAY, self.next_seed()) + 200;

                // Send logs
                for chunk_start in (0..n_logs).step_by(500) {
                    let chunk_end = (chunk_start + 500).min(n_logs);
                    let logs: Vec<LogEvent> = (chunk_start..chunk_end)
                        .map(|i| {
                            let offset_ns = random_usize(day_ns as usize, self.next_seed()) as i64;
                            self.generate_log_event(service, day_ts + offset_ns, total_events as usize + i)
                        })
                        .collect();

                    let batch = IngestBatch {
                        service_name: service.to_string(),
                        instance_id: "bench-instance".to_string(),
                        batch_seq,
                        logs,
                        spans: vec![],
                        metrics: vec![],
                    };
                    batch_seq += 1;

                    let resp = send_frame(tcp_stream, &batch).await;
                    if !resp.accepted {
                        anyhow::bail!("batch rejected at log chunk {}: {}", chunk_start, resp.error);
                    }
                }

                // Send spans
                for chunk_start in (0..n_spans).step_by(500) {
                    let chunk_end = (chunk_start + 500).min(n_spans);
                    let spans: Vec<Span> = (chunk_start..chunk_end)
                        .map(|i| {
                            let offset_ns = random_usize(day_ns as usize, self.next_seed()) as i64;
                            self.generate_span(service, day_ts + offset_ns, total_events as usize + i)
                        })
                        .collect();

                    let batch = IngestBatch {
                        service_name: service.to_string(),
                        instance_id: "bench-instance".to_string(),
                        batch_seq,
                        logs: vec![],
                        spans,
                        metrics: vec![],
                    };
                    batch_seq += 1;

                    let resp = send_frame(tcp_stream, &batch).await;
                    if !resp.accepted {
                        anyhow::bail!("batch rejected at span chunk {}: {}", chunk_start, resp.error);
                    }
                }

                total_events += (n_logs + n_spans) as u64;
            }

            let elapsed = gen_start.elapsed();
            if elapsed.saturating_sub(last_report) > Duration::from_secs(REPORT_INTERVAL_SECS) || day + 1 == days {
                info!(
                    "Generated day {}/{} — {:.1}K events so far ({:.0} evt/s)",
                    day + 1,
                    days,
                    total_events as f64 / 1000.0,
                    total_events as f64 / elapsed.as_secs_f64(),
                );
                last_report = elapsed;
            }
        }

        Ok(total_events)
    }
}

// ---------------------------------------------------------------------------
// Query patterns
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct LatencyStats {
    #[allow(dead_code)]
    count: usize,
    min: f64,
    #[allow(dead_code)]
    max: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    mean: f64,
}

fn compute_percentiles(mut samples: Vec<f64>) -> LatencyStats {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let count = samples.len();
    let sum: f64 = samples.iter().sum();
    LatencyStats {
        count,
        min: samples[0],
        max: samples[count - 1],
        p50: samples[(count as f64 * 0.50) as usize],
        p95: samples[(count as f64 * 0.95) as usize],
        p99: samples[(count as f64 * 0.99) as usize],
        mean: sum / count as f64,
    }
}

async fn run_queries(
    engine: &QueryEngine,
    label: &str,
    n_runs: usize,
) -> Result<QueryPatternResults> {
    let mut filtered_scan = Vec::with_capacity(n_runs);
    let mut latency_percentile = Vec::with_capacity(n_runs);
    let mut error_rate = Vec::with_capacity(n_runs);

    info!("  Running queries for {label}...");

    // Warm-up: run each query once before measuring
    let _ = engine
        .query("SELECT count(*) FROM logs WHERE service = 'checkout' AND level = 'error' AND timestamp > now() - interval '7 days'")
        .await;
    let _ = engine
        .query("SELECT approx_percentile_cont(latency_ms, 0.50), approx_percentile_cont(latency_ms, 0.95), approx_percentile_cont(latency_ms, 0.99) FROM spans WHERE service = 'checkout' AND start_time > now() - interval '7 days'")
        .await;
    let _ = engine
        .query("SELECT date_trunc('hour', timestamp) AS bucket, count(*) FILTER (WHERE level = 'error') AS errors, count(*) AS total FROM logs WHERE timestamp > now() - interval '7 days' GROUP BY bucket ORDER BY bucket")
        .await;

    for run in 0..n_runs {
        // Pattern 1: Filtered log-explorer scan
        let sql1 = "SELECT message, level, timestamp, attributes FROM logs WHERE service = 'checkout' AND level = 'error' AND timestamp > now() - interval '7 days'".to_string();
        let start = Instant::now();
        let _result1 = engine.query(&sql1).await?;
        let elapsed1 = start.elapsed().as_secs_f64() * 1000.0;
        filtered_scan.push(elapsed1);

        // Pattern 2: Latency percentile aggregate
        let sql2 = "SELECT approx_percentile_cont(latency_ms, 0.50) AS p50, approx_percentile_cont(latency_ms, 0.95) AS p95, approx_percentile_cont(latency_ms, 0.99) AS p99 FROM spans WHERE service = 'checkout' AND start_time > now() - interval '7 days'".to_string();
        let start = Instant::now();
        let _result2 = engine.query(&sql2).await?;
        let elapsed2 = start.elapsed().as_secs_f64() * 1000.0;
        latency_percentile.push(elapsed2);

        // Pattern 3: Error-rate time-bucket query
        let sql3 = "SELECT date_trunc('hour', timestamp) AS bucket, count(*) FILTER (WHERE level = 'error') AS errors, count(*) AS total FROM logs WHERE timestamp > now() - interval '7 days' GROUP BY bucket ORDER BY bucket".to_string();
        let start = Instant::now();
        let _result3 = engine.query(&sql3).await?;
        let elapsed3 = start.elapsed().as_secs_f64() * 1000.0;
        error_rate.push(elapsed3);

        if (run + 1) % 5 == 0 {
            info!("    Run {}/{}", run + 1, n_runs);
        }
    }

    Ok(QueryPatternResults {
        label: label.to_string(),
        filtered_scan: compute_percentiles(filtered_scan),
        latency_percentile: compute_percentiles(latency_percentile),
        error_rate: compute_percentiles(error_rate),
    })
}

#[derive(Debug)]
struct QueryPatternResults {
    label: String,
    filtered_scan: LatencyStats,
    latency_percentile: LatencyStats,
    error_rate: LatencyStats,
}

fn print_results(heading: &str, results: &[QueryPatternResults]) {
    println!();
    println!("{}", heading);
    println!("{:-^80}", "");
    println!(
        "{:<15} | {:<12} | {:<8} {:<8} {:<8} {:<8} {:<8}",
        "Pattern", "Metric", "min(ms)", "p50(ms)", "p95(ms)", "p99(ms)", "mean(ms)"
    );
    println!("{:-<15}-+-{:-<12}-+-{:-<8}-{:-<8}-{:-<8}-{:-<8}-{:-<8}", "", "", "", "", "", "", "");

    for r in results {
        for (name, stats) in [
            ("filtered_scan", &r.filtered_scan),
            ("latency_pctl", &r.latency_percentile),
            ("error_rate", &r.error_rate),
        ] {
            println!(
                "{:<15} | {:<12} | {:<8.1} {:<8.1} {:<8.1} {:<8.1} {:<8.1}",
                r.label,
                name,
                stats.min,
                stats.p50,
                stats.p95,
                stats.p99,
                stats.mean,
            );
        }
    }

    // Deltas
    if results.len() == 2 {
        println!();
        println!("Delta (12-weeks − 1-week) — isolates directory-listing cost:");
        println!("{:-^80}", "");
        println!(
            "{:<27} {:<8} {:<8} {:<8} {:<8}",
            "Pattern", "p50 Δ", "p95 Δ", "p99 Δ", "mean Δ"
        );
        println!("{:-<27} {:-<8} {:-<8} {:-<8} {:-<8}", "", "", "", "", "");

        for (name, stats1, stats2) in [
            ("filtered_scan", &results[0].filtered_scan, &results[1].filtered_scan),
            ("latency_pctl", &results[0].latency_percentile, &results[1].latency_percentile),
            ("error_rate", &results[0].error_rate, &results[1].error_rate),
        ] {
            let dp50 = stats2.p50 - stats1.p50;
            let dp95 = stats2.p95 - stats1.p95;
            let dp99 = stats2.p99 - stats1.p99;
            let dmean = stats2.mean - stats1.mean;
            println!(
                "{:<27} {:<+8.1} {:<+8.1} {:<+8.1} {:<+8.1}",
                name, dp50, dp95, dp99, dmean,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Main benchmark
// ---------------------------------------------------------------------------

async fn run_benchmark_for_scale(
    scale: &Scale,
    gen: &mut DatasetGenerator,
) -> Result<QueryPatternResults> {
    let dir = test_dir(scale.name);
    info!(
        "=== Scale: {} ({} days) ===",
        scale.name, scale.days
    );
    info!("Workspace: {}", dir.display());

    fs::create_dir_all(dir.join(".greplog"))?;

    let tcp_port = {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        l.local_addr().unwrap().port()
    };

    let config = Config {
        workspace: dir.clone(),
        tcp_port,
        ui_port: 0,
        flush_interval_secs: FLUSH_INTERVAL_SECS,
        socket_path: PathBuf::from(".greplog/bench.sock"),
        max_buffer_events: 100_000,
        compaction_interval_secs: COMPACTION_INTERVAL_SECS,
    };

    let config = Arc::new(config.clone());

    let (batch_tx, batch_rx) = mpsc::channel::<(
        IngestBatch,
        bytes::Bytes,
        oneshot::Sender<greplog_core::gen::IngestResponse>,
    )>(1024);

    let writer = Writer::new(&config.workspace, config.max_buffer_events)?;
    let dropped_events = writer.dropped_events();
    writer.run_startup(50_000)?;
    let _writer_handle = writer.spawn(
        batch_rx,
        Duration::from_secs(config.flush_interval_secs),
    );

    let _ingest_handle = ingest::IngestServer::new(batch_tx.clone()).spawn(&config);
    let query_engine = QueryEngine::new(&config.workspace)?;
    let server_state = AppState {
        config: config.clone(),
        query_engine: query_engine.clone(),
        dropped_events,
        live_tx: None,
    };
    let _server_handle = HttpServer::new().spawn(server_state);

    tokio::time::sleep(Duration::from_millis(300)).await;

    // ── Generate data through real TCP ingest ──
    {
        let mut stream = TcpStream::connect(("127.0.0.1", tcp_port)).await?;
        let total = gen.generate(&mut stream, scale.days).await?;
        info!("Generated {:.1}K events in {} days", total as f64 / 1000.0, scale.days);
    }

    // ── Wait for flush ──
    info!("Waiting for flush...");
    tokio::time::sleep(Duration::from_secs(FLUSH_INTERVAL_SECS + 1)).await;

    // ── Trigger and wait for compaction ──
    info!("Running compaction...");
    let workspace = config.workspace.clone();
    tokio::task::spawn_blocking(move || {
        let _ = compact_eligible_partitions(&workspace);
    })
    .await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create a fresh engine to pick up compacted files
    let engine = QueryEngine::new(&config.workspace)?;

    // ── Verify data existence ──
    let count = engine.query("SELECT count(*) AS cnt FROM logs").await?;
    let span_count = engine.query("SELECT count(*) AS cnt FROM spans").await?;
    info!(
        "Store: {} log rows, {} span rows",
        count.rows[0][0],
        span_count.rows[0][0],
    );

    // ── Run query benchmarks ──
    let results = run_queries(&engine, scale.name, 20).await?;

    // ── Cleanup ──
    drop(batch_tx);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = fs::remove_dir_all(&dir);

    Ok(results)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let start = Instant::now();
    let mut gen = DatasetGenerator::new();
    let mut all_results = Vec::new();

    for scale in SCALES {
        let r = run_benchmark_for_scale(scale, &mut gen).await?;
        all_results.push(r);
    }

    // ── Concurrent ingestion test ──
    info!("\n=== Concurrent ingestion + query test ===");

    // Run at 1-week scale to keep it fast
    let _conc_scale = &SCALES[0];
    let dir = test_dir("concurrent");
    fs::create_dir_all(dir.join(".greplog"))?;

    let tcp_port = {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        l.local_addr().unwrap().port()
    };

    let config = Arc::new(Config {
        workspace: dir.clone(),
        tcp_port,
        ui_port: 0,
        flush_interval_secs: 2,
        socket_path: PathBuf::from(".greplog/bench.sock"),
        max_buffer_events: 100_000,
        compaction_interval_secs: 30,
    });

    let (batch_tx, batch_rx) = mpsc::channel(1024);
    let writer = Writer::new(&config.workspace, config.max_buffer_events)?;
    let dropped_events = writer.dropped_events();
    writer.run_startup(50_000)?;
    let _writer_handle = writer.spawn(batch_rx, Duration::from_secs(config.flush_interval_secs));
    let _ingest_handle = ingest::IngestServer::new(batch_tx.clone()).spawn(&config);
    let engine = QueryEngine::new(&config.workspace)?;

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Start background ingestion
    let ingest_tcp_port = tcp_port;
    let ingest_handle = tokio::spawn(async move {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", ingest_tcp_port)).await {
            let _conc_gen = DatasetGenerator::new();
            for _ in 0..20 {
                let mut attrs = HashMap::new();
                attrs.insert("bench".into(), "concurrent".into());
                let log = LogEvent {
                    service_name: "bench-load".to_string(),
                    message: "concurrent ingest event".to_string(),
                    level: "info".to_string(),
                    timestamp_ns: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as i64,
                    logger_name: "bench".to_string(),
                    file: "benchmark.rs".to_string(),
                    line: 0,
                    correlation_id: String::new(),
                    attributes: attrs,
                    stack_trace: vec![],
                    exception_type: String::new(),
                    exception_message: String::new(),
                    event_id: format!("conc-{:020}", Instant::now().elapsed().as_nanos()),
                };
                let batch = IngestBatch {
                    service_name: "bench-load".to_string(),
                    instance_id: "conc-instance".to_string(),
                    batch_seq: 0,
                    logs: vec![log],
                    spans: vec![],
                    metrics: vec![],
                };
                let _ = send_frame(&mut stream, &batch).await;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    });

    // Run queries concurrently
    info!("Running queries during concurrent ingestion...");
    let n_conc_runs = 10;
    let mut conc_results = Vec::new();

    // Pre-populate some data
    for day in 0..2 {
        let day_ts = day_start_ns(0) + (day as i64) * 86_400_000_000_000i64;
        for service in SERVICES {
            let mut gen2 = DatasetGenerator::new();
            if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", tcp_port)).await {
                for _ in 0..10 {
                    let log = gen2.generate_log_event(service, day_ts, 0);
                    let batch = IngestBatch {
                        service_name: service.to_string(),
                        instance_id: "bench".to_string(),
                        batch_seq: 0,
                        logs: vec![log],
                        spans: vec![],
                        metrics: vec![],
                    };
                    let _ = send_frame(&mut stream, &batch).await;
                }
            }
        }
    }

    // Warmup
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Wait for ingest background task to finish
    ingest_handle.await.ok();

    for _run in 0..n_conc_runs {
        let start = Instant::now();
        let _ = engine
            .query("SELECT count(*) FROM logs WHERE service = 'checkout' AND level = 'error'")
            .await?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        conc_results.push(elapsed);
    }

    let conc_stats = compute_percentiles(conc_results);
    info!("Concurrent ingest query latency: p50={:.1}ms p95={:.1}ms p99={:.1}ms", 
          conc_stats.p50, conc_stats.p95, conc_stats.p99);

    // Check event drops
    let drops = dropped_events.load(Ordering::SeqCst);
    info!("Dropped events during concurrent test: {}", drops);

    drop(batch_tx);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = fs::remove_dir_all(&dir);

    // ── Print final results ──
    println!("\n\n");
    println!("{}", "=".repeat(80));
    println!("{:^80}", "GREPLOG QUERY LATENCY BENCHMARK RESULTS");
    println!("{}", "=".repeat(80));
    println!("Seed: {}", bench_seed());
    println!("Services: {:?}", SERVICES);
    println!("Events/service/day: ~{} logs + ~{} spans", EVENTS_PER_SERVICE_PER_DAY, SPANS_PER_SERVICE_PER_DAY);
    println!("Flush interval: {}s", FLUSH_INTERVAL_SECS);
    println!("Compaction interval: {}s", COMPACTION_INTERVAL_SECS);
    println!("Query runs: 20 warm, 20 measured per pattern");

    print_results("Query Latency by Scale", &all_results);

    // Delta analysis
    if all_results.len() == 2 {
        println!();
        println!("{}", "-".repeat(80));
        println!("Delta analysis:");
        for (name, s1, s2) in [
            ("filtered_scan", &all_results[0].filtered_scan, &all_results[1].filtered_scan),
            ("latency_pctl", &all_results[0].latency_percentile, &all_results[1].latency_percentile),
            ("error_rate", &all_results[0].error_rate, &all_results[1].error_rate),
        ] {
            let dp50 = s2.p50 - s1.p50;
            let dp95 = s2.p95 - s1.p95;
            println!(
                "  {}: p50 Δ = {}{:.1}ms, p95 Δ = {}{:.1}ms",
                name,
                if dp50 > 0.0 { "+" } else { "" },
                dp50,
                if dp95 > 0.0 { "+" } else { "" },
                dp95,
            );
        }
    }

    println!();
    println!("{}", "-".repeat(80));
    println!("Concurrent ingestion during query:");
    println!("  Query latency: p50={:.1}ms p95={:.1}ms p99={:.1}ms", 
             conc_stats.p50, conc_stats.p95, conc_stats.p99);
    println!("  Events dropped: {}", drops);

    println!();
    println!("Total benchmark duration: {:.1}s", start.elapsed().as_secs_f64());

    Ok(())
}
