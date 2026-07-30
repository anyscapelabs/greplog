use anyhow::Result;
use clap::Parser;
use greplog_core::gen::{IngestBatch, IngestResponse, LogEvent};
use prost::Message;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::interval;
use tracing::{error, info, warn};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = ".greplog/greplog.sock")]
    socket: String,

    #[arg(long, default_value = "127.0.0.1:4317")]
    host: String,

    #[arg(long, default_value_t = 100)]
    rate: u64,

    #[arg(long, default_value_t = 10)]
    events: usize,

    #[arg(long, default_value_t = 30)]
    duration: u64,

    #[arg(long, default_value = "load-test")]
    service: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let socket_path = args.socket;
    let host = args.host;
    let target_rate = args.rate;
    let events_per_batch = args.events;
    let test_duration = Duration::from_secs(args.duration);
    let service = args.service;

    info!(
        "Starting load test: rate={}/s, events/batch={}, duration={:?}, service={}",
        target_rate, events_per_batch, test_duration, service
    );

    let accepted = Arc::new(AtomicU64::new(0));
    let rejected = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    let start = Instant::now();

    let send_handle = {
        let accepted = accepted.clone();
        let rejected = rejected.clone();
        let running = running.clone();
        let socket_path = socket_path.clone();
        let service = service.clone();
        tokio::spawn(async move {
            let stream = match UnixStream::connect(&socket_path).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to connect to UDS {}: {}", socket_path, e);
                    running.store(false, Ordering::Relaxed);
                    return;
                }
            };
            let (mut reader, mut writer) = tokio::io::split(stream);

            let mut interval = interval(Duration::from_micros(1_000_000 / target_rate));
            interval.tick().await;

            while running.load(Ordering::Relaxed) {
                interval.tick().await;

                let batch = make_batch(&service, events_per_batch);
                let encoded = batch.encode_to_vec();
                let len = (encoded.len() as u32).to_le_bytes();
                if let Err(e) = writer.write_all(&len).await {
                    error!("Write error: {}", e);
                    break;
                }
                if let Err(e) = writer.write_all(&encoded).await {
                    error!("Write error: {}", e);
                    break;
                }

                let mut len_buf = [0u8; 4];
                if let Err(e) = reader.read_exact(&mut len_buf).await {
                    error!("Read len error: {}", e);
                    break;
                }
                let resp_len = u32::from_le_bytes(len_buf) as usize;
                let mut resp_buf = vec![0u8; resp_len];
                if let Err(e) = reader.read_exact(&mut resp_buf).await {
                    error!("Read body error: {}", e);
                    break;
                }
                if let Ok(resp) = IngestResponse::decode(&resp_buf[..]) {
                    if resp.accepted {
                        accepted.fetch_add(resp.events_count as u64, Ordering::Relaxed);
                    } else {
                        rejected.fetch_add(events_per_batch as u64, Ordering::Relaxed);
                    }
                }
            }
        })
    };

    let status_handle = {
        let running = running.clone();
        let host = host.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let mut report_interval = interval(Duration::from_secs(5));
            report_interval.tick().await;

            while running.load(Ordering::Relaxed) {
                report_interval.tick().await;
                let url = format!("http://{}/status", host);
                match client.get(&url).send().await {
                    Ok(resp) => {
                        match resp.json::<serde_json::Value>().await {
                            Ok(json) => {
                                let dropped = json["dropped_events"].as_u64().unwrap_or(0);
                                let active = json["active_queries"].as_u64().unwrap_or(0);
                                let buffer = json["buffer_occupancy"].as_u64().unwrap_or(0);
                                info!(
                                    "STATUS: dropped_events={}, active_queries={}, buffer_occupancy={}",
                                    dropped, active, buffer
                                );
                            }
                            Err(e) => error!("Failed to parse status JSON: {}", e),
                        }
                    }
                    Err(e) => error!("Failed to fetch status: {}", e),
                }
            }
        })
    };

    tokio::time::sleep(test_duration).await;
    running.store(false, Ordering::Relaxed);

    let _ = send_handle.await;
    let _ = status_handle.await;

    let elapsed = start.elapsed();
    let accepted_total = accepted.load(Ordering::Relaxed);
    let rejected_total = rejected.load(Ordering::Relaxed);
    let throughput = if elapsed.as_secs_f64() > 0.0 {
        accepted_total as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };

    info!("=== Load Test Results ===");
    info!("Duration: {:?}", elapsed);
    info!("Accepted events: {}", accepted_total);
    info!("Rejected events: {}", rejected_total);
    info!("Throughput: {:.0} events/sec", throughput);
    info!("Target rate: {}/s batches ({} events/s)", target_rate, target_rate * events_per_batch as u64);

    if rejected_total > 0 {
        warn!("DROPS DETECTED: {} events were rejected!", rejected_total);
    }

    Ok(())
}

fn make_batch(service: &str, events: usize) -> IngestBatch {
    let mut batch = IngestBatch {
        service_name: service.to_string(),
        instance_id: "load-test-instance".to_string(),
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
            message: format!("load-test log message {}", i),
            level: "info".to_string(),
            timestamp_ns: now_ns + i as i64,
            logger_name: "load-test".to_string(),
            file: "load_test.rs".to_string(),
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
