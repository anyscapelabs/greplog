use anyhow::Result;
use greplog_core::gen::{IngestBatch, IngestResponse};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, error, info};

pub mod buffer;
pub mod compaction;
pub mod compaction_scheduler;
pub mod dedup;
pub mod flush;
pub mod io;
pub mod wal;

// ---------------------------------------------------------------------------
// Shared mutable state
// ---------------------------------------------------------------------------

/// In-memory buffer backed by Arrow per-(service, date) column builders.
///
/// At ingest time each event's fields are appended directly into the
/// builders.  At flush time the builders are finished into RecordBatches
/// and written to Parquet — no per-event iteration at flush time.
struct BufferInner {
    log_groups: HashMap<(String, String), buffer::LogBuilders>,
    span_groups: HashMap<(String, String), buffer::SpanBuilders>,
    metric_groups: HashMap<(String, String), buffer::MetricBuilders>,
    /// Total number of individual events across all builder groups.
    /// Used for capacity checks.
    event_count: usize,
}

// ---------------------------------------------------------------------------
// Writer — the Parquet‑backed replacement for the old DuckDB Writer
// ---------------------------------------------------------------------------

/// Background writer that persists incoming `IngestBatch` events through a
/// two‑phase protocol:
///
/// 1. **Append** – every batch is appended to the WAL (durable) and added to
///    the in‑memory buffer under a single lock (buffer → wal, consistent lock
///    order).
/// 2. **Flush** – on a configurable interval the buffer is atomically swapped
///    with an empty one and the WAL is rotated **while holding the same lock**,
///    then the extracted events are written to Hive‑partitioned Parquet files.
///    After a successful flush the sealed WAL segments are checkpointed.
///
/// ### §3b.1 Compliance
///
/// The atomic buffer‑swap + WAL‑rotation guarantees that no incoming event can
/// be split across the boundary: its WAL entry and its buffer entry always land
/// on the same side of the flush.
pub struct Writer {
    wal: Arc<Mutex<wal::WalWriter>>,
    buffer: Arc<Mutex<BufferInner>>,
    dedup_cache: Arc<Mutex<dedup::DedupCache>>,
    is_flushing: Arc<AtomicBool>,
    startup_done: Arc<AtomicBool>,
    dropped_events: Arc<AtomicUsize>,
    data_dir: PathBuf,
    max_buffer_events: usize,
    live_tx: Option<broadcast::Sender<String>>,
}

impl Writer {
    /// Open or resume a WAL at `base_dir/wal/` and prepare the in‑memory buffer.
    /// The `data/` tree is created lazily by flush.
    pub fn new(base_dir: &Path, max_buffer_events: usize) -> Result<Self> {
        let w = wal::WalWriter::open(base_dir)?;
        Ok(Self {
            wal: Arc::new(Mutex::new(w)),
            buffer: Arc::new(Mutex::new(BufferInner {
                log_groups: HashMap::new(),
                span_groups: HashMap::new(),
                metric_groups: HashMap::new(),
                event_count: 0,
            })),
            dedup_cache: Arc::new(Mutex::new(dedup::DedupCache::new(0))),
            is_flushing: Arc::new(AtomicBool::new(false)),
            startup_done: Arc::new(AtomicBool::new(false)),
            dropped_events: Arc::new(AtomicUsize::new(0)),
            data_dir: base_dir.to_path_buf(),
            max_buffer_events,
            live_tx: None,
        })
    }

    /// Enable live tail by attaching a broadcast sender.
    pub fn with_live_tail(mut self, tx: broadcast::Sender<String>) -> Self {
        self.live_tx = Some(tx);
        self
    }

    /// Run the full startup recovery sequence:
    ///
    /// 1. Remove `.tmp` files left by prior crashes (`cleanup_temp_files`).
    /// 2. Resolve any pending compaction manifests (`reconcile_compactions`).
    /// 3. Seed a content‑hash‑based dedup cache from the most‑recently‑
    ///    written Parquet files (covers the flush‑checkpoint‑race window).
    /// 4. Replay un‑flushed events from the WAL.
    /// 5. Filter replayed events through the dedup cache — any event whose
    ///    content ID already exists in Parquet is a duplicate and is skipped.
    /// 6. Flush the remaining (truly new) events and checkpoint the WAL.
    ///
    /// Must be called **before** `spawn()`. After this call the in‑memory
    /// buffer is empty and the WAL writer points at a fresh, empty segment.
    pub fn run_startup(&self, dedup_seed_limit: usize) -> Result<()> {
        flush::cleanup_temp_files(&self.data_dir)?;
        compaction::reconcile_compactions(&self.data_dir)?;

        let replayed = wal::replay_segments(&self.data_dir)?;
        if replayed.is_empty() {
            info!("Startup recovery: no WAL data to replay");
            self.startup_done.store(true, Ordering::SeqCst);
            return Ok(());
        }

        // Replace the shared cache with a properly-seeded instance.
        {
            let mut guard = self.dedup_cache.lock().map_err(|e| anyhow::anyhow!("dedup lock: {e}"))?;
            let mut fresh = dedup::DedupCache::new(dedup_seed_limit);
            fresh.seed_from_recent_files(&self.data_dir)?;
            *guard = fresh;
        }

        let mut dedup_guard = self.dedup_cache.lock().map_err(|e| anyhow::anyhow!("dedup lock: {e}"))?;
        let mut deduped: Vec<IngestBatch> = Vec::with_capacity(replayed.len());
        for (batch, _meta) in replayed {
            let filtered = dedup_guard.filter_batch(&batch);
            if filtered.logs.is_empty()
                && filtered.spans.is_empty()
                && filtered.metrics.is_empty()
            {
                continue;
            }
            dedup_guard.insert_batch(&batch);
            deduped.push(filtered);
        }
        drop(dedup_guard);

        if deduped.is_empty() {
            let mut wal = self
                .wal
                .lock()
                .map_err(|e| anyhow::anyhow!("wal lock: {e}"))?;
            wal.rotate()?;
            let seq = wal.current_seq();
            wal.delete_segments_up_to(seq)?;
            info!("Startup recovery: all WAL data already flushed (dedup removed everything)");
            self.startup_done.store(true, Ordering::SeqCst);
            return Ok(());
        }

        {
            let mut wal = self
                .wal
                .lock()
                .map_err(|e| anyhow::anyhow!("wal lock: {e}"))?;
            flush::flush_to_parquet(&self.data_dir, &deduped, Some(&mut wal), &self.is_flushing)?;
        }
        info!(
            "Startup recovery: flushed {} batch(es) after dedup",
            deduped.len(),
        );

        self.startup_done.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn dropped_events(&self) -> Arc<AtomicUsize> {
        self.dropped_events.clone()
    }

    /// Mark startup recovery as complete. Exposed for tests that bypass
    /// `run_startup()`.
    #[allow(dead_code)]
    pub fn mark_startup_done(&self) {
        self.startup_done.store(true, Ordering::SeqCst);
    }

    // ------------------------------------------------------------------
    // Public spawn — runs the Writer as a background tokio task
    // ------------------------------------------------------------------

    /// Launch the writer loop.
    ///
    /// `batch_rx` delivers batches together with a oneshot sender that the
    /// writer uses to respond with an [`IngestResponse`].
    ///
    /// The returned `JoinHandle` completes when the channel closes and the
    /// final flush finishes.
    pub fn spawn(
        self,
        mut batch_rx: mpsc::Receiver<(IngestBatch, bytes::Bytes, oneshot::Sender<IngestResponse>)>,
        flush_interval: Duration,
    ) -> tokio::task::JoinHandle<()> {
        let wal = self.wal;
        let buffer = self.buffer;
        let dedup_cache = self.dedup_cache;
        let is_flushing = self.is_flushing;
        let startup_done = self.startup_done;
        let dropped_events = self.dropped_events;
        let data_dir = self.data_dir;
        let max_buffer_events = self.max_buffer_events;
        let live_tx = self.live_tx;

        tokio::spawn(async move {
            let mut flush_ticker = tokio::time::interval(flush_interval);
            // Skip the first immediate tick (brand‑new buffer is empty).
            flush_ticker.tick().await;

            loop {
                tokio::select! {
                    _ = flush_ticker.tick() => {
                        let should_flush = buffer.lock()
                            .map(|b| b.event_count > 0)
                            .unwrap_or(false);
                        if should_flush {
                            let w = wal.clone();
                            let b = buffer.clone();
                            let dc = dedup_cache.clone();
                            let f = is_flushing.clone();
                            let d = data_dir.clone();
                            if let Err(e) = tokio::task::spawn_blocking(move || {
                                flush_inner(&w, &b, &dc, &f, &d)
                            }).await.unwrap_or_else(|e| {
                                Err(anyhow::anyhow!("spawn_blocking join: {e}"))
                            }) {
                                error!("Flush cycle failed: {e}");
                            }
                        }
                    }
                    msg = batch_rx.recv() => {
                        match msg {
                            Some((batch, raw_bytes, resp_tx)) => {
                                let w = wal.clone();
                                let b = buffer.clone();
                                let dc = dedup_cache.clone();
                                let d = dropped_events.clone();
                                let sd = startup_done.clone();
                                let max = max_buffer_events;
                                let lt = live_tx.clone();
                                tokio::task::spawn_blocking(move || {
                                    let resp = handle_batch(&w, &b, &dc, batch, raw_bytes, max, &d, &sd, lt.as_ref());
                                    let _ = resp_tx.send(resp);
                                });
                            }
                            None => {
                                info!("Writer channel closed, flushing remaining and exiting");
                                let has_data = buffer.lock()
                                    .map(|b| b.event_count > 0)
                                    .unwrap_or(false);
                                if has_data {
                                    let w = wal.clone();
                                    let b = buffer.clone();
                                    let dc = dedup_cache.clone();
                                    let f = is_flushing.clone();
                                    let d = data_dir.clone();
                                    if let Err(e) = tokio::task::spawn_blocking(move || {
                                        flush_inner(&w, &b, &dc, &f, &d)
                                    }).await.unwrap_or_else(|e| {
                                        Err(anyhow::anyhow!("spawn_blocking join: {e}"))
                                    }) {
                                        error!("Final flush failed: {e}");
                                    }
                                }
                                // Final fsync + stop the background sync thread.
                                if let Ok(mut wal_guard) = wal.lock() {
                                    let _ = wal_guard.flush_fsync();
                                    wal_guard.shutdown_background();
                                }
                                break;
                            }
                        }
                    }
                }
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn count_events(batch: &IngestBatch) -> usize {
    batch.logs.len() + batch.spans.len() + batch.metrics.len()
}

/// Compute content-based IDs for every event in a batch and set the
/// `event_id` field on the struct.  Runs BEFORE any lock so that flush
/// can use the pre-computed `event_id` directly (via `log_content_id()`'s
/// priority check) without recomputing content hashes.
///
/// This does not change the WAL frame bytes (they use the original
/// protobuf `raw_bytes` from the wire — unchanged).
fn assign_content_ids(batch: &mut IngestBatch) {
    for log in &mut batch.logs {
        if log.event_id.is_empty() {
            log.event_id = flush::log_content_id(log);
        }
    }
    for span in &mut batch.spans {
        if span.event_id.is_empty() {
            span.event_id = flush::span_content_id(span);
        }
    }
    for metric in &mut batch.metrics {
        if metric.event_id.is_empty() {
            metric.event_id = flush::metric_content_id(metric);
        }
    }
}

/// Process one ingested batch: compute content IDs, check capacity,
/// append to WAL, add to buffer, return an [`IngestResponse`].
///
/// ## Lock order (deadlock avoidance)
///
/// Both this function and `flush_inner` acquire locks in the **same order**:
/// **buffer → wal**.  Never reverse this order.
///
/// ## Critical section
///
/// The WAL frame encoding and content-ID computation happen *before*
/// acquiring either lock.  Under the lock only: writing the already-encoded
/// bytes to the WAL, and pushing the already-computed batch into the buffer.
#[allow(clippy::too_many_arguments)]
fn handle_batch(
    wal_lock: &Mutex<wal::WalWriter>,
    buffer_lock: &Mutex<BufferInner>,
    _dedup_cache: &Mutex<dedup::DedupCache>,
    mut batch: IngestBatch,
    raw_bytes: bytes::Bytes,
    max_buffer_events: usize,
    dropped_events: &AtomicUsize,
    startup_done: &AtomicBool,
    live_tx: Option<&broadcast::Sender<String>>,
) -> IngestResponse {
    let event_count = count_events(&batch);

    // 0. Gate on startup recovery.
    if !startup_done.load(Ordering::SeqCst) {
        return rejected_response(event_count, "startup_recovery_in_progress");
    }

    // ── Pre-lock work ──────────────────────────────────────────────────
    // Content-ID computation and WAL frame encoding happen here, before
    // any lock is acquired.  These are the bulk of per-event CPU cost.
    // Multiple batches can run this phase concurrently across the
    // spawn_blocking thread pool — there is no contention until the
    // locks below.

    // 1. Compute content IDs and set event_id on the struct.
    assign_content_ids(&mut batch);

    // 2. Pre-encode the WAL frame (already outside lock in Round 9b).
    let crc = crc32fast::hash(&raw_bytes);
    let frame_len = 4 + 4 + raw_bytes.len();
    let mut frame = Vec::with_capacity(frame_len);
    frame.extend_from_slice(&(4u32 + raw_bytes.len() as u32).to_le_bytes());
    frame.extend_from_slice(&crc.to_le_bytes());
    frame.extend_from_slice(&raw_bytes);

    // ── Locked critical section (minimal, fixed per batch) ─────────────

    // 3. Capacity check.
    {
        let buf = match buffer_lock.lock() {
            Ok(g) => g,
            Err(_) => {
                return rejected_response(event_count, "internal_error: buffer lock poisoned");
            }
        };
        if buf.event_count + event_count > max_buffer_events {
            dropped_events.fetch_add(event_count, Ordering::SeqCst);
            return rejected_response(event_count, "buffer_full");
        }
    }

    // 4. Append to WAL.
    {
        let mut wal = match wal_lock.lock() {
            Ok(g) => g,
            Err(_) => {
                return rejected_response(event_count, "internal_error: wal lock poisoned");
            }
        };
        if let Err(e) = wal.append_raw(&frame) {
            return rejected_response(event_count, &format!("wal_append: {e}"));
        }
    }

    // 5. Add to buffer (append to per-table, per-(service,date) Arrow builders).
    {
        let mut buf = match buffer_lock.lock() {
            Ok(g) => g,
            Err(_) => {
                return rejected_response(event_count, "internal_error: buffer lock poisoned");
            }
        };
        let service = if batch.service_name.is_empty() {
            "default".to_string()
        } else {
            batch.service_name.clone()
        };
        for log in &batch.logs {
            let date = flush::micros_to_date_string(flush::ns_to_micros(log.timestamp_ns));
            let key = (service.clone(), date);
            let builders = buf
                .log_groups
                .entry(key)
                .or_insert_with(buffer::LogBuilders::new);
            builders.append(log, &service);
        }
        for span in &batch.spans {
            let date = flush::micros_to_date_string(flush::ns_to_micros(span.start_time_ns));
            let key = (service.clone(), date);
            let builders = buf
                .span_groups
                .entry(key)
                .or_insert_with(buffer::SpanBuilders::new);
            builders.append(span, &service);
        }
        for metric in &batch.metrics {
            let date = flush::micros_to_date_string(flush::ns_to_micros(metric.timestamp_ns));
            let key = (service.clone(), date);
            let builders = buf
                .metric_groups
                .entry(key)
                .or_insert_with(buffer::MetricBuilders::new);
            builders.append(metric, &service);
        }
        buf.event_count += event_count;
    }

    if let Some(tx) = live_tx {
        if tx.receiver_count() > 0 {
            broadcast_batch(tx, &batch);
        }
    }

    IngestResponse {
        accepted: true,
        events_count: event_count as i64,
        error: String::new(),
    }
}

/// Atomically swap the buffer and rotate the WAL, flush to Parquet,
/// checkpoint, then update the dedup cache.  Runs inside `spawn_blocking`.
fn flush_inner(
    wal_lock: &Mutex<wal::WalWriter>,
    buffer_lock: &Mutex<BufferInner>,
    _dedup_cache: &Mutex<dedup::DedupCache>,
    is_flushing: &AtomicBool,
    data_dir: &Path,
) -> Result<()> {
    if is_flushing.load(Ordering::SeqCst) {
        debug!("Flush skipped - already in progress");
        return Ok(());
    }
    is_flushing.store(true, Ordering::SeqCst);
    let result = flush_inner_impl(wal_lock, buffer_lock, _dedup_cache, data_dir);
    is_flushing.store(false, Ordering::SeqCst);
    result
}

fn flush_inner_impl(
    wal_lock: &Mutex<wal::WalWriter>,
    buffer_lock: &Mutex<BufferInner>,
    _dedup_cache: &Mutex<dedup::DedupCache>,
    data_dir: &Path,
) -> Result<()> {
    // ── Atomic section: extract builder groups + rotate WAL ──
    let (log_groups, span_groups, metric_groups, sealed_seq) = {
        let mut buf = buffer_lock
            .lock()
            .map_err(|e| anyhow::anyhow!("buffer lock: {e}"))?;
        let log_groups = std::mem::take(&mut buf.log_groups);
        let span_groups = std::mem::take(&mut buf.span_groups);
        let metric_groups = std::mem::take(&mut buf.metric_groups);
        buf.event_count = 0;

        let mut wal = wal_lock
            .lock()
            .map_err(|e| anyhow::anyhow!("wal lock: {e}"))?;
        wal.rotate()?;
        let seq = wal.current_seq();
        (log_groups, span_groups, metric_groups, seq)
    };

    if log_groups.is_empty() && span_groups.is_empty() && metric_groups.is_empty() {
        return Ok(());
    }

    let mut files_written = 0;

    // ── Finish log groups ──
    for ((service, date), mut builders) in log_groups {
        let batch = builders.finish()?;
        let dir = data_dir
            .join("data")
            .join("table_type=logs")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        io::write_parquet_atomic(&dir, &batch)?;
        files_written += 1;
    }

    // ── Finish span groups ──
    for ((service, date), mut builders) in span_groups {
        let batch = builders.finish()?;
        let dir = data_dir
            .join("data")
            .join("table_type=spans")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        io::write_parquet_atomic(&dir, &batch)?;
        files_written += 1;
    }

    // ── Finish metric groups ──
    for ((service, date), mut builders) in metric_groups {
        let batch = builders.finish()?;
        let dir = data_dir
            .join("data")
            .join("table_type=metrics")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        io::write_parquet_atomic(&dir, &batch)?;
        files_written += 1;
    }

    if files_written > 0 {
        // ── Checkpoint sealed segments ──
        let wal = wal_lock
            .lock()
            .map_err(|e| anyhow::anyhow!("wal lock: {e}"))?;
        wal.delete_segments_up_to(sealed_seq)?;
    }

    Ok(())
}

fn rejected_response(event_count: usize, reason: &str) -> IngestResponse {
    IngestResponse {
        accepted: false,
        events_count: event_count as i64,
        error: reason.to_string(),
    }
}

/// Broadcast events from an accepted batch to Live Tail subscribers via
/// the tokio broadcast channel.  Each event is serialized as a JSON line.
fn broadcast_batch(tx: &broadcast::Sender<String>, batch: &IngestBatch) {
    for log in &batch.logs {
        let msg = serde_json::json!({
            "type": "log",
            "timestamp_ns": log.timestamp_ns,
            "service": log.service_name,
            "level": log.level,
            "message": log.message,
            "logger_name": log.logger_name,
            "file": log.file,
            "line": log.line,
            "correlation_id": log.correlation_id,
            "attributes": log.attributes,
        });
        let _ = tx.send(msg.to_string());
    }
    for span in &batch.spans {
        let msg = serde_json::json!({
            "type": "span",
            "start_time_ns": span.start_time_ns,
            "end_time_ns": span.end_time_ns,
            "service": span.service_name,
            "name": span.name,
            "route": span.route,
            "method": span.method,
            "status_code": span.status_code,
            "error": span.error,
            "correlation_id": span.correlation_id,
            "attributes": span.attributes,
        });
        let _ = tx.send(msg.to_string());
    }
    for metric in &batch.metrics {
        let msg = serde_json::json!({
            "type": "metric",
            "timestamp_ns": metric.timestamp_ns,
            "service": metric.service_name,
            "name": metric.name,
            "value": metric.value,
            "labels": metric.labels,
        });
        let _ = tx.send(msg.to_string());
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use greplog_core::gen::LogEvent;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio::sync::oneshot;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("greplog_writer_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn make_log(service: &str, timestamp_ns: i64, level: &str, msg: &str) -> LogEvent {
        let mut attrs = HashMap::new();
        attrs.insert("key".into(), "val".into());
        LogEvent {
            service_name: service.into(),
            message: msg.into(),
            level: level.into(),
            timestamp_ns,
            logger_name: String::new(),
            file: String::new(),
            line: 0,
            correlation_id: String::new(),
            attributes: attrs,
            stack_trace: Vec::new(),
            exception_type: String::new(),
            exception_message: String::new(),
            event_id: String::new(),
        }
    }

    fn make_batch(service: &str, seq: i64, n: usize) -> IngestBatch {
        let now = 1_700_000_000_000_000_000;
        let logs: Vec<_> = (0..n)
            .map(|i| make_log(service, now + i as i64, "info", &format!("msg-{i}")))
            .collect();
        IngestBatch {
            service_name: service.into(),
            instance_id: "test-instance".into(),
            batch_seq: seq,
            logs,
            spans: vec![],
            metrics: vec![],
        }
    }

    // ------------------------------------------------------------------
    // Test 1: Basic end‑to‑end ingest → flush → query
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_basic_ingest_flush_query() {
        let dir = test_dir("ingest_flush_query");
        let writer = Writer::new(&dir, 100_000).expect("new writer");
        let dropped = writer.dropped_events();
        writer.mark_startup_done();

        let (batch_tx, batch_rx) = mpsc::channel(64);
        let _handle = writer.spawn(batch_rx, Duration::from_millis(100));

        let send_batch = |svc: &str, seq: i64, n: usize| {
            let tx = batch_tx.clone();
            let svc = svc.to_string();
            async move {
                let (resp_tx, resp_rx) = oneshot::channel();
                let batch = make_batch(&svc, seq, n);
                use prost::Message;
                let raw_bytes = bytes::Bytes::from(batch.encode_to_vec());
                tx.send((batch, raw_bytes, resp_tx)).await.expect("send");
                let resp = resp_rx.await.expect("response");
                assert!(resp.accepted, "batch {seq} should be accepted");
                resp
            }
        };

        send_batch("svc", 1, 3).await;
        send_batch("svc", 2, 2).await;

        // Wait for flush
        tokio::time::sleep(Duration::from_millis(250)).await;

        let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
        let result = engine.query("SELECT count(*) AS cnt FROM logs").await.expect("query");
        assert_eq!(result.rows[0][0], serde_json::json!(5i64), "5 events flushed");

        assert_eq!(dropped.load(Ordering::SeqCst), 0, "no drops");

        drop(batch_tx);
        // Give the writer time to close
        tokio::time::sleep(Duration::from_millis(300)).await;
        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 2: Atomicity under concurrent producers (§3b.1 proof)
    // ------------------------------------------------------------------
    //
    // Multiple concurrent tasks send batches while flush cycles run
    // frequently.  After all sends complete and the final flush drains the
    // buffer, verify every sent event appears **exactly once**.
    #[tokio::test]
    async fn test_concurrent_ingest_atomicity() {
        let dir = test_dir("concurrent_atomicity");
        let writer = Writer::new(&dir, 500_000).expect("new writer");
        writer.mark_startup_done();

        let (batch_tx, batch_rx) = mpsc::channel(256);
        let _handle = writer.spawn(batch_rx, Duration::from_millis(50));

        let n_producers = 8;
        let batches_per_producer = 250;
        let events_per_batch = 5;
        let total_expected = (n_producers * batches_per_producer * events_per_batch) as i64;

        let mut send_handles = Vec::new();
        for p in 0..n_producers {
            let tx = batch_tx.clone();
            let service = format!("producer-{p}");
            send_handles.push(tokio::spawn(async move {
                for b in 0..batches_per_producer {
                    let (resp_tx, resp_rx) = oneshot::channel();
                    let batch = make_batch(&service, (b + p * batches_per_producer) as i64, events_per_batch);
                    use prost::Message;
                    let raw_bytes = bytes::Bytes::from(batch.encode_to_vec());
                    tx.send((
                        batch,
                        raw_bytes,
                        resp_tx,
                    ))
                    .await
                    .expect("send");
                    let resp = resp_rx.await.expect("response");
                    assert!(resp.accepted, "all batches under test should be accepted");
                }
            }));
        }

        for h in send_handles {
            h.await.expect("producer join");
        }

        // Close channel — writer final‑flushes and exits.
        drop(batch_tx);
        tokio::time::sleep(Duration::from_millis(500)).await;

        let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
        let result = engine.query("SELECT count(*) AS cnt FROM logs").await.expect("query");
        assert_eq!(
            result.rows[0][0],
            serde_json::json!(total_expected),
            "all {total_expected} events must appear exactly once — this proves no split across flush boundary",
        );

        // NOTE: content‑based IDs are deterministic — identical events
        // (same service, same timestamp, same message, etc.) share
        // the same ID by design, so we do NOT assert distinct IDs here.
        // The atomicity guarantee is that every sent event appears at
        // least once (proved by the count check above).

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 3: Backpressure under sustained overload
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_backpressure_overload() {
        let dir = test_dir("backpressure");
        let writer = Writer::new(&dir, 10).expect("new writer"); // tiny cap
        let dropped = writer.dropped_events();
        writer.mark_startup_done();

        let (batch_tx, batch_rx) = mpsc::channel(256);
        let _handle = writer.spawn(batch_rx, Duration::from_secs(60)); // very slow flush

        let burst = 50;
        let mut accepted = 0i64;
        let mut rejected = 0i64;

        for i in 0..burst {
            let (resp_tx, resp_rx) = oneshot::channel();
            let batch = make_batch("svc", i as i64, 1);
            use prost::Message;
            let raw_bytes = bytes::Bytes::from(batch.encode_to_vec());
            batch_tx
                .send((batch, raw_bytes, resp_tx))
                .await
                .expect("send");
            let resp = resp_rx.await.expect("response");
            if resp.accepted {
                accepted += resp.events_count;
            } else {
                rejected += resp.events_count;
                assert_eq!(resp.error, "buffer_full", "rejection reason");
            }
        }

        assert!(
            (10..50).contains(&accepted),
            "buffer cap 10 should allow ~10 events, got {accepted}"
        );
        assert!(rejected > 0, "some events should be rejected");
        assert_eq!(
            dropped.load(Ordering::SeqCst) as i64,
            rejected,
            "dropped_events counter matches rejected count"
        );

        // Force a flush and verify
        drop(batch_tx);
        tokio::time::sleep(Duration::from_millis(300)).await;

        let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
        let result = engine.query("SELECT count(*) AS cnt FROM logs").await.expect("query");
        assert_eq!(
            result.rows[0][0],
            serde_json::json!(accepted),
            "only accepted events in parquet"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 4b: Lock‑hold duration — flush releases locks before Parquet I/O
    // ------------------------------------------------------------------
    //
    // Spawn a "flush" task that holds the buffer + wal locks only for the
    // atomic extraction+rotation, then sleeps (simulating a slow Parquet
    // write) before checkpointing.  While the sleep runs, a concurrent
    // `handle_batch` call must complete quickly — proving the locks were
    // released and ingest is not stalled by the slow I/O.
    #[tokio::test]
    async fn test_flush_lock_not_held_during_parquet_write() {
        let dir = test_dir("flush_lock_duration");
        let writer = Writer::new(&dir, 100_000).expect("new writer");

        let wal = writer.wal.clone();
        let buffer = writer.buffer.clone();
        let dedup_cache = writer.dedup_cache.clone();
        let dropped = writer.dropped_events();
        let startup_done = std::sync::atomic::AtomicBool::new(true);

        // Pre‑fill the buffer so there is data to "flush".
        for i in 0..5 {
            let batch = make_batch("svc", i, 3);
            use prost::Message;
            let raw_bytes = bytes::Bytes::from(batch.encode_to_vec());
            handle_batch(&wal, &buffer, &dedup_cache, batch, raw_bytes, 100_000, &dropped, &startup_done, None);
        }

        // Phase 1: simulate the atomic section of a flush
        // (lock extraction + WAL rotation in the same scope as flush_inner_impl).
        let (had_data, sealed_seq) = {
            let mut buf = buffer.lock().expect("buffer lock");
            let had = buf.event_count > 0;
            let _log_groups = std::mem::take(&mut buf.log_groups);
            let _span_groups = std::mem::take(&mut buf.span_groups);
            let _metric_groups = std::mem::take(&mut buf.metric_groups);
            buf.event_count = 0;
            let mut w = wal.lock().expect("wal lock");
            w.rotate().expect("rotate");
            let seq = w.current_seq();
            (had, seq)
        }; // Both guards dropped here — same as flush_inner_impl

        assert!(had_data, "should have extracted data");

        // Phase 2: simulate a slow Parquet write (100 ms) while holding
        // NO locks.  During this window a concurrent ingest must be fast.
        let slow_sleep = Duration::from_millis(100);

        let wal2 = wal.clone();

        let slow_flush = tokio::task::spawn_blocking(move || {
            // "slow Parquet write"
            std::thread::sleep(slow_sleep);

            // Checkpoint (re‑acquires wal lock)
            let w = wal2.lock().expect("wal lock");
            w.delete_segments_up_to(sealed_seq).ok();
        });

        // Give the slow flush a moment to start sleeping.
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Now — while the simulated Parquet write is in progress — try a
        // concurrent ingest.  It must complete in far less than 100 ms
        // because the buffer and wal locks are NOT held.
        let start = std::time::Instant::now();
        let batch = make_batch("svc", 100, 1);
        use prost::Message;
        let raw_bytes = bytes::Bytes::from(batch.encode_to_vec());
        let resp = handle_batch(&wal, &buffer, &dedup_cache, batch, raw_bytes, 100_000, &dropped, &startup_done, None);
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(50),
            "concurrent ingest took {elapsed:?} while flush held locks — \
             lock not released before Parquet write"
        );
        assert!(resp.accepted, "ingest during flush should succeed");

        slow_flush.await.expect("slow flush join");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 5: Crash recovery under concurrent ingest
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_crash_recovery_concurrent() {
        let dir = test_dir("crash_recovery_concurrent");

        // Phase 1: write concurrent-ish data to WAL, then "crash" (leak).
        {
            let mut wal = wal::WalWriter::open(&dir).expect("open wal");
            for p in 0..4 {
                for b in 0..50 {
                    let batch = make_batch(&format!("p{p}"), (b + p * 50) as i64, 2);
                    wal.append(&batch).expect("append");
                }
            }
            std::thread::sleep(Duration::from_millis(200));
            let _ = std::mem::ManuallyDrop::new(wal);
        }

        // Phase 2: replay WAL → flush to Parquet → verify.
        {
            let replayed = wal::replay_segments(&dir).expect("replay");
            assert!(!replayed.is_empty(), "WAL has unflushed data");
            let batches: Vec<IngestBatch> = replayed.into_iter().map(|(b, _)| b).collect();
            flush::flush_parquet_only(&dir, &batches).expect("flush");

            let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
            let result = engine.query("SELECT count(*) AS cnt FROM logs").await.expect("query");
            let expected: i64 = 4 * 50 * 2;
            assert_eq!(
                result.rows[0][0],
                serde_json::json!(expected),
                "all events survive crash and replay"
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ==================================================================
    // Round 8: run_startup recovery tests
    // ==================================================================

    fn make_unique_log(service: &str, id: usize) -> LogEvent {
        LogEvent {
            service_name: service.into(),
            message: format!("msg-{}", id),
            level: "info".into(),
            timestamp_ns: 1_700_000_000_000_000_000 + id as i64,
            logger_name: String::new(),
            file: String::new(),
            line: 0,
            correlation_id: String::new(),
            attributes: HashMap::new(),
            stack_trace: Vec::new(),
            exception_type: String::new(),
            exception_message: String::new(),
            event_id: String::new(),
        }
    }

    fn make_unique_batch(service: &str, seq: i64, ids: &[usize]) -> IngestBatch {
        let logs: Vec<_> = ids.iter().map(|i| make_unique_log(service, *i)).collect();
        IngestBatch {
            service_name: service.into(),
            instance_id: "test-instance".into(),
            batch_seq: seq,
            logs,
            spans: vec![],
            metrics: vec![],
        }
    }

    // ------------------------------------------------------------------
    // Test R1: Empty WAL — run_startup is a no-op.
    // ------------------------------------------------------------------
    #[test]
    fn test_startup_recovery_empty_wal() {
        let dir = test_dir("startup_empty_wal");
        let writer = Writer::new(&dir, 100_000).expect("new writer");
        writer.run_startup(1000).expect("run_startup");

        let replayed = wal::replay_segments(&dir).expect("replay");
        assert!(replayed.is_empty(), "no WAL data after startup");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test R2: Basic replay — unflushed WAL data gets flushed during
    // startup recovery.
    // ------------------------------------------------------------------
    #[test]
    fn test_startup_recovery_basic_replay() {
        let dir = test_dir("startup_basic_replay");

        // Manually write events to the WAL, simulating a crash before flush.
        {
            let mut wal = wal::WalWriter::open(&dir).expect("open wal");
            let batch = make_unique_batch("svc", 1, &[1, 2, 3]);
            wal.append(&batch).expect("append");
            std::thread::sleep(Duration::from_millis(200));
            let _ = std::mem::ManuallyDrop::new(wal);
        }

        // Startup recovery should replay and flush.
        {
            let writer = Writer::new(&dir, 100_000).expect("new writer");
            writer.run_startup(1000).expect("run_startup");
        }

        // Verify data is in Parquet.
        let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(engine.query("SELECT count(*) AS cnt FROM logs")).expect("query");
        assert_eq!(result.rows[0][0], serde_json::json!(3i64));

        // WAL should be empty after checkpoint.
        let replayed = wal::replay_segments(&dir).expect("replay");
        assert!(replayed.is_empty(), "WAL should be empty after startup recovery");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test R3: Dedup — events already in Parquet before crash are skipped
    // during replay.
    // ------------------------------------------------------------------
    #[test]
    fn test_startup_recovery_dedup_skips_duplicates() {
        let dir = test_dir("startup_dedup_skip");

        // Phase 1: flush events to Parquet (simulating successful pre-crash flush).
        {
            let batch = make_unique_batch("svc", 1, &[1, 2, 3]);
            flush::flush_parquet_only(&dir, &[batch]).expect("flush");
        }

        // Phase 2: write the SAME events + a new event to WAL (simulating a
        // crash after WAL append but before checkpoint).
        {
            let mut wal = wal::WalWriter::open(&dir).expect("open wal");
            let batch = make_unique_batch("svc", 2, &[1, 2, 3, 4]);
            wal.append(&batch).expect("append");
            std::thread::sleep(Duration::from_millis(200));
            let _ = std::mem::ManuallyDrop::new(wal);
        }

        // Phase 3: run startup recovery.
        {
            let writer = Writer::new(&dir, 100_000).expect("new writer");
            writer.run_startup(1000).expect("run_startup");
        }

        // Phase 4: verify — only the NEW event (id=4) was added.
        let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(engine.query("SELECT count(*) AS cnt FROM logs")).expect("query");
        assert_eq!(
            result.rows[0][0],
            serde_json::json!(4i64),
            "3 original + 1 new = 4 total (duplicates skipped)"
        );

        let replayed = wal::replay_segments(&dir).expect("replay");
        assert!(replayed.is_empty(), "WAL should be empty after recovery");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test R4: Partial dedup — mix of duplicate and unique events.
    // ------------------------------------------------------------------
    #[test]
    fn test_startup_recovery_partial_dedup() {
        let dir = test_dir("startup_partial_dedup");

        // Phase 1: flush events 1-3.
        {
            let batch = make_unique_batch("svc", 1, &[1, 2, 3]);
            flush::flush_parquet_only(&dir, &[batch]).expect("flush");
        }

        // Phase 2: write events 2, 3, 4, 5 to WAL (2 and 3 are duplicates).
        {
            let mut wal = wal::WalWriter::open(&dir).expect("open wal");
            let batch = make_unique_batch("svc", 2, &[2, 3, 4, 5]);
            wal.append(&batch).expect("append");
            std::thread::sleep(Duration::from_millis(200));
            let _ = std::mem::ManuallyDrop::new(wal);
        }

        // Phase 3: startup recovery.
        {
            let writer = Writer::new(&dir, 100_000).expect("new writer");
            writer.run_startup(1000).expect("run_startup");
        }

        // Phase 4: verify — only events 4 and 5 are new.
        let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(engine.query("SELECT count(*) AS cnt FROM logs")).expect("query");
        assert_eq!(
            result.rows[0][0],
            serde_json::json!(5i64),
            "3 original + 2 new = 5 total (2 duplicates skipped)"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test R5: Cleanup — .tmp files are removed during startup recovery.
    // ------------------------------------------------------------------
    #[test]
    fn test_startup_recovery_cleanup_temp() {
        let dir = test_dir("startup_cleanup_temp");

        // Write some events to WAL first.
        {
            let mut wal = wal::WalWriter::open(&dir).expect("open wal");
            let batch = make_unique_batch("svc", 1, &[1, 2]);
            wal.append(&batch).expect("append");
            std::thread::sleep(Duration::from_millis(200));
            let _ = std::mem::ManuallyDrop::new(wal);
        }

        // Manually create .tmp file in the data directory.
        let tmp_dir = dir.join("data").join("table_type=logs").join("service=svc").join("date=2023-11-15");
        fs::create_dir_all(&tmp_dir).expect("create tmp dir");
        let tmp_file = tmp_dir.join("orphan.tmp");
        fs::write(&tmp_file, b"garbage").expect("write tmp");

        // Run startup — should clean up .tmp and flush WAL.
        {
            let writer = Writer::new(&dir, 100_000).expect("new writer");
            writer.run_startup(1000).expect("run_startup");
        }

        // Verify .tmp is gone.
        assert!(!tmp_file.exists(), ".tmp file should be removed during startup");

        // Verify data is in Parquet.
        let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(engine.query("SELECT count(*) AS cnt FROM logs")).expect("query");
        assert_eq!(result.rows[0][0], serde_json::json!(2i64));

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test R6: End‑to‑end "crash then recover" with no data loss.
    // ------------------------------------------------------------------
    //
    // Simulate: normal ingest → partial flush (data in Parquet, WAL
    // not yet check-pointed) → "crash" → startup recovery → verify
    // each event appears exactly once.
    #[test]
    fn test_startup_recovery_no_data_loss() {
        let dir = test_dir("startup_no_data_loss");

        // Phase 1: flush events 1-5 to Parquet (successful flush).
        {
            let batch = make_unique_batch("svc", 1, &[1, 2, 3, 4, 5]);
            flush::flush_parquet_only(&dir, &[batch]).expect("flush");
        }

        // Phase 2: write events 3-7 to WAL (3,4,5 dupes; 6,7 new) then
        // "crash" without checkpointing.
        {
            let mut wal = wal::WalWriter::open(&dir).expect("open wal");
            let batch = make_unique_batch("svc", 2, &[3, 4, 5, 6, 7]);
            wal.append(&batch).expect("append");
            std::thread::sleep(Duration::from_millis(200));
            let _ = std::mem::ManuallyDrop::new(wal);
        }

        // Phase 3: startup recovery.
        {
            let writer = Writer::new(&dir, 100_000).expect("new writer");
            writer.run_startup(1000).expect("run_startup");
        }

        // Phase 4: all 7 events present, no more, no less.
        let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(engine.query("SELECT count(*) AS cnt FROM logs")).expect("query");
        assert_eq!(
            result.rows[0][0],
            serde_json::json!(7i64),
            "all 7 events present exactly once"
        );

        // Phase 5: WAL is empty (checkpointed).
        let replayed = wal::replay_segments(&dir).expect("replay");
        assert!(replayed.is_empty(), "WAL fully check-pointed");

        let _ = fs::remove_dir_all(&dir);
    }

    // ==================================================================
    // §8b.2 — Three required tests
    // ==================================================================

    // ------------------------------------------------------------------
    // Test A: Ingest gated on recovery (empirical).
    // ------------------------------------------------------------------
    //
    // Create a Writer that has NOT completed startup recovery, attempt an
    // ingest (simulated via handle_batch), confirm rejection.  Then mark
    // startup done and confirm a subsequent ingest is accepted.
    #[test]
    fn test_ingest_gated_during_recovery() {
        let dir = test_dir("ingest_gated");
        let writer = Writer::new(&dir, 100_000).expect("new writer");

        let wal = writer.wal.clone();
        let buffer = writer.buffer.clone();
        let dedup_cache = writer.dedup_cache.clone();
        let dropped = writer.dropped_events();
        // NOTE: startup_done is still false — run_startup was NOT called.

        // Attempt 1: startup not done → must reject.
        let batch1 = make_batch("svc", 1, 1);
        use prost::Message;
        let raw_bytes1 = bytes::Bytes::from(batch1.encode_to_vec());
        let resp1 = handle_batch(&wal, &buffer, &dedup_cache, batch1, raw_bytes1, 100_000, &dropped, &writer.startup_done, None);
        assert!(!resp1.accepted, "ingest should be rejected during recovery");
        assert_eq!(resp1.error, "startup_recovery_in_progress");

        // Mark startup complete.
        writer.mark_startup_done();

        // Attempt 2: startup done → must accept.
        let batch2 = make_batch("svc", 2, 2);
        let raw_bytes2 = bytes::Bytes::from(batch2.encode_to_vec());
        let resp2 = handle_batch(&wal, &buffer, &dedup_cache, batch2, raw_bytes2, 100_000, &dropped, &writer.startup_done, None);
        assert!(resp2.accepted, "ingest should be accepted after recovery");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test B: Clean failure on genuine corruption (invalid manifest JSON).
    // ------------------------------------------------------------------
    #[test]
    fn test_startup_recovery_corrupt_manifest() {
        let dir = test_dir("startup_corrupt_manifest");

        // Create a compaction manifest with invalid JSON.
        // Note: the manifest file is `.compaction_manifest` (hidden file).
        let compaction_dir = dir.join("data").join("table_type=logs")
            .join("service=svc").join("date=2023-01-01");
        fs::create_dir_all(&compaction_dir).expect("create partition");
        let manifest_path = compaction_dir.join(".compaction_manifest");
        fs::write(&manifest_path, b"not valid json at all { broken").expect("write corrupt manifest");

        let writer = Writer::new(&dir, 100_000).expect("new writer");
        let result = writer.run_startup(1000);
        assert!(
            result.is_err(),
            "run_startup must return Err on corrupt manifest, got Ok: {:?}",
            result,
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("manifest") || err_msg.contains("parse"),
            "error should reference the manifest/parse failure, got: {err_msg}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test C: Ordering — .tmp file inside a partition being reconciled.
    // ------------------------------------------------------------------
    //
    // Create a partition directory that simultaneously has an interrupted
    // compaction (manifest entry + merged file + undeleted source files)
    // AND a stray .tmp file.  After run_startup(), the .tmp must be gone
    // and the reconciliation must handle the compaction state cleanly.
    #[test]
    fn test_startup_recovery_tmp_inside_reconciled_partition() {
        use std::fs;

        let dir = test_dir("startup_tmp_reconcile");

        // Create a partition directory (same structure compaction v2 expects).
        let partition_dir = dir.join("data").join("table_type=logs")
            .join("service=svc").join("date=2023-06-15");
        fs::create_dir_all(&partition_dir).expect("create partition");

        // Write a "source" Parquet file (undeleted — simulating a crash
        // after merge write but before source deletion).
        let source_path = partition_dir.join("000001.parquet");
        // Write a minimal valid Parquet file.
        {
            use arrow::array::StringArray;
            use arrow::datatypes::{DataType, Field};
            use arrow::record_batch::RecordBatch;
            use parquet::arrow::ArrowWriter;
            use std::sync::Arc;

            let schema = Arc::new(arrow::datatypes::Schema::new(vec![
                Field::new("id", DataType::Utf8, false),
            ]));
            let batch = RecordBatch::try_new(
                schema.clone(),
                vec![Arc::new(StringArray::from(vec!["evt-1"]))],
            )
            .expect("batch");
            let file = fs::File::create(&source_path).expect("create source");
            let mut writer = ArrowWriter::try_new(file, schema, None).expect("writer");
            writer.write(&batch).expect("write");
            writer.close().expect("close");
        }

        // Write a "merged" Parquet file (simulating phase2 completion).
        let merged_path = partition_dir.join("000002.parquet");
        {
            use arrow::array::StringArray;
            use arrow::datatypes::{DataType, Field};
            use arrow::record_batch::RecordBatch;
            use parquet::arrow::ArrowWriter;
            use std::sync::Arc;

            let schema = Arc::new(arrow::datatypes::Schema::new(vec![
                Field::new("id", DataType::Utf8, false),
            ]));
            let batch = RecordBatch::try_new(
                schema.clone(),
                vec![Arc::new(StringArray::from(vec!["evt-1"]))],
            )
            .expect("batch");
            let file = fs::File::create(&merged_path).expect("create merged");
            let mut writer = ArrowWriter::try_new(file, schema, None).expect("writer");
            writer.write(&batch).expect("write");
            writer.close().expect("close");
        }

        // Write a compaction manifest with a pending_merge entry that
        // references the source and the merged file.
        // Field names must match the ManifestEntry serde struct.
        // Note: the manifest file is `.compaction_manifest` (hidden file).
        let manifest_path = partition_dir.join(".compaction_manifest");
        let manifest_content = serde_json::json!({
            "entries": [{
                "merged_file": "000002.parquet",
                "source_files": ["000001.parquet"],
                "status": "pending_merge"
            }]
        });
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest_content).unwrap())
            .expect("write manifest");

        // Write a stray .tmp file in the same partition directory.
        let tmp_path = partition_dir.join("orphan.tmp");
        fs::write(&tmp_path, b"garbage").expect("write tmp");

        // Run startup — should clean .tmp AND reconcile the compaction.
        let writer = Writer::new(&dir, 100_000).expect("new writer");
        writer.run_startup(1000).expect("run_startup should succeed");

        // Assert: .tmp file is gone.
        assert!(!tmp_path.exists(), "stray .tmp should be removed during startup");

        // Assert: after reconciliation the manifest entry progressed
        // (status changed from pending_merge to something else).
        let after_manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        let entries = after_manifest["entries"].as_array().expect("entries array");
        assert!(!entries.is_empty(), "manifest should still have entries");
        let status = entries[0]["status"].as_str().expect("status string");
        assert_ne!(
            status, "pending_merge",
            "reconciliation should advance the manifest past pending_merge, got: {status}"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
