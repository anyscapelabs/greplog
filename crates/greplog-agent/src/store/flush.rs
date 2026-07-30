use anyhow::Result;
use arrow::array::*;

use arrow::record_batch::RecordBatch;
use greplog_core::arrow_schema;
use greplog_core::gen::{IngestBatch, LogEvent, Metric, Span};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const DATA_DIR: &str = "data";

/// Result of a flush cycle.
#[derive(Debug, Default)]
pub struct FlushResult {
    #[allow(dead_code)]
    pub files_written: usize,
}

// ---------------------------------------------------------------------------
// Content‑based IDs (deterministic, no external dependencies)
// ---------------------------------------------------------------------------

/// Return a stable ID for a LogEvent.
///
/// Priority order (§1.3b):
/// 1. Client-supplied `event_id` if non-empty.
/// 2. Deterministic content-hash fallback from key fields.
pub fn log_content_id(log: &LogEvent) -> String {
    if !log.event_id.is_empty() {
        return log.event_id.clone();
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    log.service_name.hash(&mut hasher);
    log.timestamp_ns.hash(&mut hasher);
    log.level.hash(&mut hasher);
    log.message.hash(&mut hasher);
    log.logger_name.hash(&mut hasher);
    log.file.hash(&mut hasher);
    log.line.hash(&mut hasher);
    log.correlation_id.hash(&mut hasher);
    let mut keys: Vec<&String> = log.attributes.keys().collect();
    keys.sort();
    for k in &keys {
        k.hash(&mut hasher);
        log.attributes[*k].hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

/// Return a stable ID for a Span.
///
/// Priority order (§1.3b):
/// 1. Client-supplied `event_id` if non-empty.
/// 2. Deterministic content-hash fallback from key fields.
pub fn span_content_id(span: &Span) -> String {
    if !span.event_id.is_empty() {
        return span.event_id.clone();
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    span.service_name.hash(&mut hasher);
    span.name.hash(&mut hasher);
    span.start_time_ns.hash(&mut hasher);
    span.end_time_ns.hash(&mut hasher);
    span.correlation_id.hash(&mut hasher);
    span.parent_correlation_id.hash(&mut hasher);
    span.route.hash(&mut hasher);
    span.method.hash(&mut hasher);
    span.status_code.hash(&mut hasher);
    span.error.hash(&mut hasher);
    let mut keys: Vec<&String> = span.attributes.keys().collect();
    keys.sort();
    for k in &keys {
        k.hash(&mut hasher);
        span.attributes[*k].hash(&mut hasher);
    }
    span.kind.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Return a stable ID for a Metric.
///
/// Priority order (§1.3b):
/// 1. Client-supplied `event_id` if non-empty.
/// 2. Deterministic content-hash fallback from key fields.
pub fn metric_content_id(metric: &Metric) -> String {
    if !metric.event_id.is_empty() {
        return metric.event_id.clone();
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    metric.service_name.hash(&mut hasher);
    metric.name.hash(&mut hasher);
    // f64 doesn't implement Hash, so use to_bits
    metric.value.to_bits().hash(&mut hasher);
    metric.timestamp_ns.hash(&mut hasher);
    let mut keys: Vec<&String> = metric.labels.keys().collect();
    keys.sort();
    for k in &keys {
        k.hash(&mut hasher);
        metric.labels[*k].hash(&mut hasher);
    }
    metric.r#type.hash(&mut hasher);
    for v in &metric.bucket_values {
        v.to_bits().hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

// ---------------------------------------------------------------------------

struct LogRow {
    id: String,
    service: String,
    timestamp: i64,
    level: Option<String>,
    route: Option<String>,
    message: Option<String>,
    attributes: Option<String>,
}

struct SpanRow {
    id: String,
    correlation_id: Option<String>,
    parent_correlation_id: Option<String>,
    service: String,
    name: Option<String>,
    route: Option<String>,
    method: Option<String>,
    status_code: Option<i32>,
    latency_ms: Option<f64>,
    is_error: Option<bool>,
    start_time: i64,
    end_time: i64,
    attributes: Option<String>,
}

struct MetricRow {
    id: String,
    service: String,
    name: String,
    timestamp: i64,
    value: f64,
    labels: Option<String>,
}

/// Flush pending batches to Parquet files, then checkpoint the WAL.
///
/// Groups events by `(service, date)` — computed from each event's
/// `timestamp_ns`/`start_time_ns` — and writes one Parquet file per
/// group using the temp-file → fsync → rename → directory-fsync
/// sequence. Empty groups produce no file.
///
/// If `wal` is provided, WAL segments containing the flushed events are
/// deleted after all files are durably written.
///
/// `is_flushing` must be an `AtomicBool` initially `false`. The function
/// sets it to `true` on entry and clears it on exit. If already `true`,
/// returns an error to prevent concurrent flushes.
pub fn flush_to_parquet(
    base_dir: &Path,
    pending: &[IngestBatch],
    wal: Option<&mut crate::store::wal::WalWriter>,
    is_flushing: &AtomicBool,
) -> Result<FlushResult> {
    if is_flushing.load(Ordering::SeqCst) {
        anyhow::bail!("flush already in progress");
    }
    is_flushing.store(true, Ordering::SeqCst);

    let result = flush_inner(base_dir, pending, wal);

    is_flushing.store(false, Ordering::SeqCst);
    result
}

/// Flush pending batches to Parquet files, skipping the concurrency
/// guard and the WAL rotation+checkpoint that `flush_to_parquet` handles.
/// The caller is responsible for both.
#[allow(dead_code)]
pub fn flush_parquet_only(base_dir: &Path, pending: &[IngestBatch]) -> Result<FlushResult> {
    flush_inner(base_dir, pending, None)
}

/// Delete any `.tmp` files found recursively under `base_dir/data/`.
/// These are left behind by a crash during the temp-file phase of a flush.
pub fn cleanup_temp_files(base_dir: &Path) -> Result<()> {
    let data_dir = base_dir.join(DATA_DIR);
    if !data_dir.exists() {
        return Ok(());
    }
    delete_tmp_recursive(&data_dir)?;
    Ok(())
}

fn delete_tmp_recursive(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            delete_tmp_recursive(&path)?;
        } else if path.extension().is_some_and(|ext| ext == "tmp") {
            let _ = fs::remove_file(&path);
        }
    }
    Ok(())
}

fn flush_inner(
    base_dir: &Path,
    pending: &[IngestBatch],
    mut wal: Option<&mut crate::store::wal::WalWriter>,
) -> Result<FlushResult> {
    // Rotate the WAL before processing so the segments containing the
    // flushed data are sealed and can be deleted after a successful flush.
    if let Some(w) = wal.as_mut() {
        w.rotate()?;
    }

    let mut log_groups: Vec<((String, String), Vec<LogRow>)> = Vec::new();
    let mut span_groups: Vec<((String, String), Vec<SpanRow>)> = Vec::new();
    let mut metric_groups: Vec<((String, String), Vec<MetricRow>)> = Vec::new();

    for batch in pending {
        let service = if batch.service_name.is_empty() {
            "default"
        } else {
            &batch.service_name
        };

        for log in &batch.logs {
            let date = micros_to_date_string(ns_to_micros(log.timestamp_ns));
            let key = (service.to_string(), date);
            let id = log_content_id(log);
            let attrs = if log.attributes.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&log.attributes).unwrap_or_default())
            };
            push_group(&mut log_groups, key, LogRow {
                id,
                service: service.to_string(),
                timestamp: ns_to_micros(log.timestamp_ns),
                level: non_empty(&log.level),
                route: None,
                message: non_empty(&log.message),
                attributes: attrs,
            });
        }

        for span in &batch.spans {
            let date = micros_to_date_string(ns_to_micros(span.start_time_ns));
            let key = (service.to_string(), date);
            let id = span_content_id(span);
            let attrs = if span.attributes.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&span.attributes).unwrap_or_default())
            };
            push_group(&mut span_groups, key, SpanRow {
                id,
                correlation_id: non_empty(&span.correlation_id),
                parent_correlation_id: non_empty(&span.parent_correlation_id),
                service: service.to_string(),
                name: non_empty(&span.name),
                route: non_empty(&span.route),
                method: Some(span.method.clone()).filter(|s| !s.is_empty()),
                status_code: Some(span.status_code).filter(|&c| c != 0),
                latency_ms: None,
                is_error: Some(!span.error.is_empty()),
                start_time: ns_to_micros(span.start_time_ns),
                end_time: ns_to_micros(span.end_time_ns),
                attributes: attrs,
            });
        }

        for metric in &batch.metrics {
            let date = micros_to_date_string(ns_to_micros(metric.timestamp_ns));
            let key = (service.to_string(), date);
            let id = metric_content_id(metric);
            let labels = if metric.labels.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&metric.labels).unwrap_or_default())
            };
            push_group(&mut metric_groups, key, MetricRow {
                id,
                service: service.to_string(),
                name: metric.name.clone(),
                timestamp: ns_to_micros(metric.timestamp_ns),
                value: metric.value,
                labels,
            });
        }
    }

    let mut files_written = 0;

    for ((service, date), rows) in &log_groups {
        let batch = build_log_batch(rows)?;
        let dir = base_dir
            .join(DATA_DIR)
            .join("table_type=logs")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        write_parquet(&dir, &batch)?;
        files_written += 1;
    }

    for ((service, date), rows) in &span_groups {
        let batch = build_span_batch(rows)?;
        let dir = base_dir
            .join(DATA_DIR)
            .join("table_type=spans")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        write_parquet(&dir, &batch)?;
        files_written += 1;
    }

    for ((service, date), rows) in &metric_groups {
        let batch = build_metric_batch(rows)?;
        let dir = base_dir
            .join(DATA_DIR)
            .join("table_type=metrics")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        write_parquet(&dir, &batch)?;
        files_written += 1;
    }

    if files_written > 0 {
        if let Some(wal) = wal {
            let checkpoint_seq = wal.current_seq();
            wal.delete_segments_up_to(checkpoint_seq)?;
        }
    }

    Ok(FlushResult { files_written })
}

fn push_group<T>(groups: &mut Vec<((String, String), Vec<T>)>, key: (String, String), row: T) {
    match groups.iter_mut().find(|(k, _)| *k == key) {
        Some((_, rows)) => rows.push(row),
        None => groups.push((key, vec![row])),
    }
}

fn build_log_batch(rows: &[LogRow]) -> Result<RecordBatch> {
    let schema = arrow_schema::log_schema();
    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.service.as_str()).collect::<Vec<_>>())),
            Arc::new(TimestampMicrosecondArray::from(rows.iter().map(|r| r.timestamp).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.level.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.route.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.message.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.attributes.as_deref()).collect::<Vec<Option<&str>>>())),
        ],
    )
    .map_err(|e| anyhow::anyhow!("build log batch: {}", e))
}

fn build_span_batch(rows: &[SpanRow]) -> Result<RecordBatch> {
    let schema = arrow_schema::span_schema();
    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.correlation_id.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.parent_correlation_id.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.service.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.name.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.route.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.method.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(Int32Array::from(rows.iter().map(|r| r.status_code).collect::<Vec<Option<i32>>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.latency_ms).collect::<Vec<Option<f64>>>())),
            Arc::new(BooleanArray::from(rows.iter().map(|r| r.is_error).collect::<Vec<Option<bool>>>())),
            Arc::new(TimestampMicrosecondArray::from(rows.iter().map(|r| r.start_time).collect::<Vec<_>>())),
            Arc::new(TimestampMicrosecondArray::from(rows.iter().map(|r| r.end_time).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.attributes.as_deref()).collect::<Vec<Option<&str>>>())),
        ],
    )
    .map_err(|e| anyhow::anyhow!("build span batch: {}", e))
}

fn build_metric_batch(rows: &[MetricRow]) -> Result<RecordBatch> {
    let schema = arrow_schema::metric_schema();
    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.service.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>())),
            Arc::new(TimestampMicrosecondArray::from(rows.iter().map(|r| r.timestamp).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.value).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.labels.as_deref()).collect::<Vec<Option<&str>>>())),
        ],
    )
    .map_err(|e| anyhow::anyhow!("build metric batch: {}", e))
}

fn write_parquet(dir: &Path, batch: &RecordBatch) -> Result<()> {
    crate::store::io::write_parquet_atomic(dir, batch)?;
    Ok(())
}

/// Convert a microsecond-precision Unix timestamp to `YYYY-MM-DD` in UTC.
pub(crate) fn micros_to_date_string(micros: i64) -> String {
    let secs = if micros >= 0 {
        micros / 1_000_000
    } else {
        (micros + 1) / 1_000_000 - 1
    };
    let days = if secs >= 0 {
        secs / 86400
    } else {
        (secs + 1) / 86400 - 1
    };

    // Convert days since 1970-01-01 to (y, m, d) using Howard Hinnant's
    // public-domain civil-date algorithm.
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let day = doy - (153 * mp + 2) / 5 + 1;
    let year = if month <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02}", year, month, day)
}

pub(crate) fn ns_to_micros(ns: i64) -> i64 {
    ns / 1000
}

pub(crate) fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() { None } else { Some(s.to_string()) }
}

#[cfg(test)]
#[path = "../../tests/modules/store/flush.rs"]
mod tests;
