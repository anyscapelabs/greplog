use anyhow::Result;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use greplog_agent::query_engine::QueryEngine;
use greplog_agent::store::compaction::compact_partition;

const TARGET_SIZE: u64 = 128 * 1024 * 1024;

fn find_date_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut queue = VecDeque::from([root.to_path_buf()]);
    while let Some(dir) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with("date=") {
                out.push(p);
            } else {
                queue.push_back(p);
            }
        }
    }
    out
}

fn count_parquet(root: &Path) -> usize {
    let mut n = 0usize;
    let mut queue = VecDeque::from([root.to_path_buf()]);
    while let Some(dir) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                queue.push_back(p);
            } else if p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".parquet"))
            {
                n += 1;
            }
        }
    }
    n
}

fn now_micros_minus(hours: u64) -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as i64;
    now - (hours as i64) * 3600 * 1_000_000
}

fn build_sql(cutoff: i64) -> Vec<(&'static str, String)> {
    vec![
        (
            "logs.count",
            format!("SELECT count(*) AS total FROM logs WHERE timestamp > to_timestamp_micros({cutoff})"),
        ),
        (
            "logs.rows",
            format!("SELECT id, timestamp, level, service, message, logger_name, file, line, correlation_id, stack_trace FROM logs WHERE timestamp > to_timestamp_micros({cutoff}) ORDER BY timestamp DESC LIMIT 1000"),
        ),
        (
            "logs.volume",
            format!("SELECT date, count(*) AS cnt FROM logs WHERE timestamp > to_timestamp_micros({cutoff}) GROUP BY date ORDER BY date"),
        ),
        (
            "dual.status_codes",
            format!("SELECT status_code, sum(cnt) AS cnt FROM ( SELECT CAST(status_code AS VARCHAR) AS status_code, count(*) AS cnt FROM spans WHERE start_time > to_timestamp_micros({cutoff}) GROUP BY CAST(status_code AS VARCHAR) UNION ALL SELECT json_get_str(attributes, 'http.status_code') AS status_code, count(*) AS cnt FROM logs WHERE logger_name = 'greplog.http' AND timestamp > to_timestamp_micros({cutoff}) GROUP BY json_get_str(attributes, 'http.status_code') ) t GROUP BY status_code ORDER BY cnt DESC"),
        ),
        (
            "dual.latency_pctl",
            format!("SELECT approx_percentile_cont(latency_ms, 0.50) AS p50, approx_percentile_cont(latency_ms, 0.90) AS p90, approx_percentile_cont(latency_ms, 0.99) AS p99 FROM ( SELECT latency_ms FROM spans WHERE start_time > to_timestamp_micros({cutoff}) UNION ALL SELECT CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) AS latency_ms FROM logs WHERE logger_name = 'greplog.http' AND timestamp > to_timestamp_micros({cutoff}) ) t"),
        ),
        (
            "logs.sparkline",
            format!("SELECT FLOOR(CAST(timestamp AS BIGINT) / 60000000) AS bucket, count(*) AS cnt FROM logs WHERE timestamp > to_timestamp_micros({cutoff}) GROUP BY 1 ORDER BY 1"),
        ),
    ]
}

async fn run_battery(engine: &QueryEngine, label: &str, sqls: &[(&str, String)]) {
    for (name, sql) in sqls {
        let mut runs: Vec<Duration> = Vec::new();
        let mut rows = 0usize;
        for _ in 0..2 {
            let start = Instant::now();
            match engine.query(sql).await {
                Ok(res) => {
                    rows = res.row_count;
                    runs.push(start.elapsed());
                }
                Err(e) => {
                    println!("[{label}] {name}: ERROR {e}");
                    runs.clear();
                    break;
                }
            }
        }
        if !runs.is_empty() {
            println!(
                "[{label}] {name}: {:.2}s | {:.2}s (cold | warm), rows={rows}",
                runs[0].as_secs_f64(),
                runs[1].as_secs_f64()
            );
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let base = std::env::args()
        .nth(1)
        .expect("usage: compact_probe <workspace-dir>");
    let base = PathBuf::from(&base);
    let data_dir = base.join("data");
    if !data_dir.exists() {
        anyhow::bail!("no data dir at {:?}", data_dir);
    }

    let engine = QueryEngine::new(&base)?;
    let cutoff = now_micros_minus(2);
    let sqls = build_sql(cutoff);

    println!(
        "workspace={:?} parquet_files={} cutoff_hours_ago=2",
        base,
        count_parquet(&data_dir)
    );

    println!("== BEFORE (unmerged) ==");
    run_battery(&engine, "before", &sqls).await;

    let leaves = find_date_dirs(&data_dir);
    println!("== MERGING {} partition dir(s) ==", leaves.len());
    for dir in &leaves {
        match compact_partition(dir, TARGET_SIZE, false) {
            Ok(res) => println!("  merged {:?}: {}+ files", dir, res.files_compacted),
            Err(e) => println!("  FAILED {:?}: {e}", dir),
        }
    }
    println!(
        "parquet_files_after_merge={}",
        count_parquet(&data_dir)
    );

    tokio::time::sleep(Duration::from_secs(12)).await;
    println!("== AFTER (merged) ==");
    run_battery(&engine, "after", &sqls).await;

    Ok(())
}
