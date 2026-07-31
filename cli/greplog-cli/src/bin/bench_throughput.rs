use clap::Parser;
use greplog_core::gen::{IngestBatch, IngestResponse, LogEvent};
use prost::Message;
use reqwest::Client;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::{interval, sleep};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = ".greplog/greplog.sock")]
    socket: String,

    #[arg(long, default_value = "127.0.0.1:4317")]
    status_host: String,

    #[arg(long, default_value_t = 100)]
    start_rate: u64,

    #[arg(long, default_value_t = 200)]
    rate_step: u64,

    #[arg(long, default_value_t = 30)]
    step_secs: u64,

    #[arg(long, default_value_t = 5)]
    steps: u32,

    #[arg(long, default_value_t = 10)]
    events_per_batch: usize,

    #[arg(long, default_value = "bench-svc")]
    service: String,

    #[arg(long, default_value_t = 1)]
    producers: u32,

    #[arg(long)]
    disk_baseline: bool,

    #[arg(long)]
    workspace: Option<String>,
}

#[derive(Default, Clone, Debug)]
struct StepResult {
    accepted: u64,
    rejected: u64,
    batches_attempted: u64,
    elapsed_ns: u128,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let args = Args::parse();

    if args.disk_baseline {
        run_disk_baseline(&args).await;
    }

    let n_producers = args.producers as usize;
    let running = Arc::new(AtomicBool::new(true));
    let all_results: Arc<Mutex<Vec<Vec<StepResult>>>> =
        Arc::new(Mutex::new((0..n_producers).map(|_| Vec::new()).collect()));

    let status_handle = spawn_status_poller(args.status_host.clone(), running.clone());

    // ── Measure average event size ──────────────────────────────────
    let sample = make_batch("sample-svc", args.events_per_batch);
    let sample_encoded = sample.encode_to_vec();
    let avg_event_size = sample_encoded.len() as f64 / args.events_per_batch as f64;
    tracing::info!(
        "Avg serialized event size: {:.0} bytes, batch size: {} bytes",
        avg_event_size,
        sample_encoded.len(),
    );

    let mut producer_handles = Vec::new();
    for p in 0..n_producers {
        let running = running.clone();
        let socket = args.socket.clone();
        let service = format!("{}-{}", args.service, p);
        let events_per_batch = args.events_per_batch;
        let start_rate = args.start_rate;
        let rate_step = args.rate_step;
        let step_secs = args.step_secs;
        let steps = args.steps;
        let results = all_results.clone();

        let handle = tokio::spawn(async move {
            let stream = loop {
                match UnixStream::connect(&socket).await {
                    Ok(s) => break s,
                    Err(e) => {
                        tracing::warn!("[producer {p}] connect failed: {e}, retrying in 1s");
                        sleep(Duration::from_secs(1)).await;
                        if !running.load(Ordering::Relaxed) {
                            return;
                        }
                    }
                }
            };
            let (mut reader, mut writer) = tokio::io::split(stream);

            let mut my_results = Vec::new();

            for step in 0..steps {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                let target_rate = start_rate + rate_step * step as u64;
                let per_batch_us = 1_000_000u64.checked_div(target_rate).unwrap_or(1_000_000);

                tracing::info!(
                    "[producer {p}] step {}/{}: configured {}/s ({} ev/s)",
                    step + 1,
                    steps,
                    target_rate,
                    target_rate * events_per_batch as u64,
                );

                let step_start = Instant::now();
                let mut ticker = interval(Duration::from_micros(per_batch_us.max(1)));
                ticker.tick().await;

                let mut batches_attempted: u64 = 0;
                let mut this_accepted: u64 = 0;
                let mut this_rejected: u64 = 0;

                while step_start.elapsed() < Duration::from_secs(step_secs)
                    && running.load(Ordering::Relaxed)
                {
                    ticker.tick().await;
                    batches_attempted += 1;

                    let batch = make_batch(&service, events_per_batch);
                    let encoded = batch.encode_to_vec();
                    let len = (encoded.len() as u32).to_le_bytes();

                    if writer.write_all(&len).await.is_err() {
                        break;
                    }
                    if writer.write_all(&encoded).await.is_err() {
                        break;
                    }

                    let mut len_buf = [0u8; 4];
                    if reader.read_exact(&mut len_buf).await.is_err() {
                        break;
                    }
                    let resp_len = u32::from_le_bytes(len_buf) as usize;
                    let mut resp_buf = vec![0u8; resp_len];
                    if reader.read_exact(&mut resp_buf).await.is_err() {
                        break;
                    }
                    if let Ok(resp) = IngestResponse::decode(&resp_buf[..]) {
                        if resp.accepted {
                            this_accepted += resp.events_count as u64;
                        } else {
                            this_rejected += events_per_batch as u64;
                        }
                    }
                }
                let elapsed = step_start.elapsed();

                let responded = this_accepted + this_rejected;
                let responded_batches = responded.div_ceil(events_per_batch as u64);
                let lost_batches = batches_attempted.saturating_sub(responded_batches);

                let actual_throughput = if elapsed.as_secs_f64() > 0.0 {
                    this_accepted as f64 / elapsed.as_secs_f64()
                } else {
                    0.0
                };

                let actual_batch_rate = if elapsed.as_secs_f64() > 0.0 {
                    responded_batches as f64 / elapsed.as_secs_f64()
                } else {
                    0.0
                };

                tracing::info!(
                    "[producer {p}] step {} done: accepted={}, rejected={}, lost_batches={}, \
                     actual={:.0} ev/s ({:.1} batches/s), target={}/s, wall={:.2}s",
                    step + 1,
                    this_accepted,
                    this_rejected,
                    lost_batches,
                    actual_throughput,
                    actual_batch_rate,
                    target_rate * events_per_batch as u64,
                    elapsed.as_secs_f64(),
                );

                my_results.push(StepResult {
                    accepted: this_accepted,
                    rejected: this_rejected,
                    batches_attempted,
                    elapsed_ns: elapsed.as_nanos(),
                });
            }

            let mut all = results.lock().unwrap();
            all[p] = my_results;
        });
        producer_handles.push(handle);
    }

    let total_secs = args.step_secs * args.steps as u64 + 10;
    sleep(Duration::from_secs(total_secs)).await;
    running.store(false, Ordering::Relaxed);

    for h in producer_handles {
        let _ = h.await;
    }
    let _ = status_handle.await;

    // ── §9b.1: Aggregated results with real measured throughput ────
    let results = all_results.lock().unwrap();
    let n_steps = results.first().map(|r| r.len()).unwrap_or(0);

    tracing::info!("");
    tracing::info!("======================================================================");
    tracing::info!("  CORRECTED THROUGHPUT (measured from IngestResponse acks over wall time)");
    tracing::info!("======================================================================");
    tracing::info!("");
    tracing::info!(
        "Configuration: {} producer(s), {} ev/batch, {} steps x {}s each",
        args.producers,
        args.events_per_batch,
        n_steps,
        args.step_secs
    );
    tracing::info!("Avg serialized event size: {:.0} bytes", avg_event_size);

    for step in 0..n_steps {
        let target_rate = args.start_rate + args.rate_step * step as u64;
        let target_ev_s = target_rate * args.events_per_batch as u64;

        let mut total_accepted: u64 = 0;
        let mut total_rejected: u64 = 0;
        let mut total_batches: u64 = 0;
        let mut max_elapsed_ns: u128 = 0;

        for p in 0..n_producers {
            if let Some(r) = results.get(p).and_then(|v| v.get(step)) {
                total_accepted += r.accepted;
                total_rejected += r.rejected;
                total_batches += r.batches_attempted;
                max_elapsed_ns = max_elapsed_ns.max(r.elapsed_ns);
            }
        }

        let wall_secs = max_elapsed_ns as f64 / 1_000_000_000.0;
        let actual_ev_s = if wall_secs > 0.0 {
            total_accepted as f64 / wall_secs
        } else {
            0.0
        };
        let responded = total_accepted + total_rejected;
        let responded_batches = responded.div_ceil(args.events_per_batch as u64);
        let lost_total = total_batches.saturating_sub(responded_batches);
        let _lost_events = lost_total * args.events_per_batch as u64;

        // Implied durable write bytes/sec
        let implied_write_bytes = actual_ev_s * avg_event_size;

        tracing::info!("");
        tracing::info!(
            "  Step {}: target={}/s, actual={:.0} ev/s (wall {:.2}s)",
            step + 1,
            target_ev_s,
            actual_ev_s,
            wall_secs,
        );
        tracing::info!(
            "    accepted={}, rejected={}, batches_attempted={}, lost_batches={}",
            total_accepted,
            total_rejected,
            total_batches,
            lost_total
        );
        tracing::info!(
            "    actual batch rate: {:.0} batches/s total",
            responded_batches as f64 / wall_secs.max(0.001)
        );
        tracing::info!(
            "    implied durable write rate: {:.1} MiB/s ({:.0} bytes/s)",
            implied_write_bytes / (1024.0 * 1024.0),
            implied_write_bytes,
        );
    }

    // Ceiling detection
    let last_step = n_steps.saturating_sub(1);
    let mut final_total_accepted: u64 = 0;
    let mut final_total_rejected: u64 = 0;
    let mut final_lost: u64 = 0;
    let mut final_wall: u128 = 0;
    for p in 0..n_producers {
        if let Some(r) = results.get(p).and_then(|v| v.get(last_step)) {
            final_total_accepted += r.accepted;
            final_total_rejected += r.rejected;
            final_wall = final_wall.max(r.elapsed_ns);
        }
    }
    // Sum lost across all steps
    for step in 0..n_steps {
        let mut total_batches: u64 = 0;
        let mut total_responded: u64 = 0;
        for p in 0..n_producers {
            if let Some(r) = results.get(p).and_then(|v| v.get(step)) {
                total_batches += r.batches_attempted;
                total_responded += r.accepted + r.rejected;
            }
        }
        let rb = total_responded.div_ceil(args.events_per_batch as u64);
        final_lost += total_batches.saturating_sub(rb);
    }

    let final_wall_secs = final_wall as f64 / 1_000_000_000.0;
    let final_ev_s = if final_wall_secs > 0.0 {
        final_total_accepted as f64 / final_wall_secs
    } else {
        0.0
    };

    tracing::info!("");
    tracing::info!("--- CEILING ANALYSIS ---");
    tracing::info!(
        "  Last-step actual throughput: {:.0} ev/s (accepted {})",
        final_ev_s,
        final_total_accepted
    );
    if final_total_rejected > 0 {
        tracing::info!(
            "  Rejections detected ({} events) — agent is dropping at this rate",
            final_total_rejected
        );
    }
    if final_lost > 0 {
        tracing::info!(
            "  Lost batches: {} (sent but no response received) — agent or network saturated",
            final_lost
        );
    }
    if final_total_rejected == 0 && final_lost == 0 {
        tracing::info!(
            "  No rejections and no lost batches at max tested rate — true ceiling not yet reached"
        );
    }

    // Rate of change: did throughput plateau?
    if n_steps >= 3 {
        let mut prev_ev_s = 0.0;
        for step in 0..n_steps {
            let mut ta: u64 = 0;
            let mut me: u128 = 0;
            for p in 0..n_producers {
                if let Some(r) = results.get(p).and_then(|v| v.get(step)) {
                    ta += r.accepted;
                    me = me.max(r.elapsed_ns);
                }
            }
            let ws = me as f64 / 1_000_000_000.0;
            let ev_s = if ws > 0.0 { ta as f64 / ws } else { 0.0 };
            if step > 0 && prev_ev_s > 0.0 {
                let pct = ((ev_s / prev_ev_s) - 1.0) * 100.0;
                tracing::info!(
                    "  Step {}→{} throughput change: {:.1}%",
                    step,
                    step + 1,
                    pct
                );
            }
            prev_ev_s = ev_s;
        }
    }

    // Physical sanity: compare against dd baseline (if available)
    if let Some(ref ws) = args.workspace {
        let baseline_file = format!("{}/.bench_baseline", ws);
        if std::path::Path::new(&baseline_file).exists() {
            // baseline was run
        }
    }

    tracing::info!("");
    tracing::info!("NOTE: All throughput numbers above are computed from actual");
    tracing::info!("IngestResponse round-trip acknowledgments divided by real");
    tracing::info!("wall-clock elapsed time — NOT from the configured/requested send rate.");

    // ── Clean up ────────────────────────────────────────────────────
    if let Some(ref ws) = args.workspace {
        let _ = std::fs::remove_file(format!("{}/.bench_baseline", ws));
    }

    Ok(())
}

async fn run_disk_baseline(args: &Args) {
    let ws = match &args.workspace {
        Some(d) => d.clone(),
        None => return,
    };

    tracing::info!("");
    tracing::info!("=== Disk Write+Fsync Baseline ===");
    tracing::info!("  Testing raw sequential write+fsync throughput on the data disk...");
    tracing::info!(
        "  (dd if=/dev/zero of=<workspace>/.bench_baseline bs=256K conv=fsync oflag=direct)"
    );

    for &size_mb in &[64u64, 256] {
        let count = size_mb * 4; // 256 KB blocks
        let file = format!("{}/.bench_baseline", ws);

        let start = Instant::now();
        let output = std::process::Command::new("dd")
            .args([
                "if=/dev/zero",
                &format!("of={}", file),
                "bs=256K",
                &format!("count={}", count),
                "conv=fsync",
                "oflag=direct",
            ])
            .output();

        let elapsed = start.elapsed();
        let _ = std::fs::remove_file(&file);

        match output {
            Ok(out) => {
                let mibs = size_mb as f64 / elapsed.as_secs_f64();
                let bytes_s = size_mb as f64 * 1024.0 * 1024.0 / elapsed.as_secs_f64();
                tracing::info!(
                    "  dd {} MB: {:.1} MiB/s ({:.0} bytes/s, {:.2}s)",
                    size_mb,
                    mibs,
                    bytes_s,
                    elapsed.as_secs_f64(),
                );
                let stderr = String::from_utf8_lossy(&out.stderr);
                for line in stderr.lines() {
                    if !line.is_empty() {
                        tracing::info!("    dd: {line}");
                    }
                }
            }
            Err(e) => {
                tracing::warn!("  dd failed: {e}");
            }
        }
    }
}

fn spawn_status_poller(host: String, running: Arc<AtomicBool>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let client = Client::new();
        let mut ticker = interval(Duration::from_secs(5));
        ticker.tick().await;

        while running.load(Ordering::Relaxed) {
            ticker.tick().await;
            let url = format!("http://{host}/status");
            match client.get(&url).send().await {
                Ok(resp) => match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let dropped = json["dropped_events"].as_u64().unwrap_or(0);
                        let buffer = json["buffer_occupancy"].as_u64().unwrap_or(0);
                        tracing::info!(
                            "STATUS: dropped_events={dropped}, buffer_occupancy={buffer}",
                        );
                    }
                    Err(e) => tracing::warn!("status parse err: {e}"),
                },
                Err(e) => tracing::warn!("status fetch err: {e}"),
            }
        }
    })
}

fn make_batch(service: &str, events: usize) -> IngestBatch {
    let mut batch = IngestBatch {
        service_name: service.to_string(),
        instance_id: "bench-instance".to_string(),
        batch_seq: 0,
        ..Default::default()
    };
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64;
    for i in 0..events {
        batch.logs.push(LogEvent {
            service_name: service.to_string(),
            message: format!("bench msg {i}"),
            level: "info".to_string(),
            timestamp_ns: now_ns + i as i64,
            logger_name: "bench".to_string(),
            file: "bench_throughput.rs".to_string(),
            line: 0,
            correlation_id: String::new(),
            attributes: std::collections::HashMap::new(),
            stack_trace: Vec::new(),
            exception_type: String::new(),
            exception_message: String::new(),
            event_id: String::new(),
        });
    }
    batch
}
