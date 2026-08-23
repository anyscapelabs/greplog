//! The dedicated OS worker threads of the write path.
//!
//! Disk I/O and columnar conversion never run on the Tokio runtime serving the
//! API. The WAL worker persists batches and `fsync`s them; the `MemTable` worker
//! converts durable records into Arrow batches, stages them in the shared
//! [`LiveBuffer`], flushes to Parquet, and signals the WAL worker to reclaim
//! covered records. The confirmation channel is separate from the ingest
//! channel so the WAL worker can always see its ingest channel close without
//! deadlocking.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use arrow::record_batch::RecordBatch;
use crossbeam::channel::{Receiver as HandoffRx, Sender as HandoffTx};
use tokio::sync::broadcast;
use tokio::sync::mpsc::{Receiver as WalRx, Sender as WalTx};

use crate::config::{EngineConfig, LiveBuffer};
use crate::error::EngineError;
use crate::ingest::{IngestBatch, WalCommand};
use crate::memtable::MemTable;
use crate::record::LogRecord;
use crate::schema::greplog_schema;
use crate::storage::ParquetFlusher;
use crate::wal::{record_count, sealed_path, sealed_segments, WalWriter};

const MAX_GROUP_BATCH: usize = 50;

struct SealedSegment {
    path: PathBuf,
    cumulative_records: u64,
}

struct WalState {
    wal: Result<WalWriter, EngineError>,
    active: PathBuf,
    sealed: Vec<SealedSegment>,
    confirmed: u64,
    cumulative: u64,
    next_seq: u64,
}

impl WalState {
    fn open(active: PathBuf) -> Self {
        let mut cumulative = 0u64;
        let mut next_seq = 1u64;
        let mut sealed = Vec::new();

        if let Ok(segments) = sealed_segments(&active) {
            for path in &segments {
                if let Ok(count) = record_count(path) {
                    cumulative += count as u64;
                    sealed.push(SealedSegment {
                        path: path.clone(),
                        cumulative_records: cumulative,
                    });
                }
                if let Some(seq) = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| name.strip_prefix("sealed-"))
                    .and_then(|rest| rest.split_once('-'))
                    .and_then(|(seq, _)| seq.parse::<u64>().ok())
                {
                    next_seq = next_seq.max(seq + 1);
                }
            }
        }
        if let Ok(count) = record_count(&active) {
            cumulative += count as u64;
        }

        let wal = WalWriter::open(&active);
        Self { wal, active, sealed, confirmed: 0, cumulative, next_seq }
    }

    fn records_written(&mut self, n: usize) {
        self.cumulative += n as u64;
    }

    fn confirm(&mut self, n: usize) {
        self.confirmed += n as u64;

        if let Some(sealed) = self.rotate() {
            self.sealed.push(SealedSegment {
                path: sealed,
                cumulative_records: self.cumulative,
            });
        }

        let confirmed = self.confirmed;
        let mut delete_through = 0usize;
        for segment in &self.sealed {
            if segment.cumulative_records <= confirmed {
                delete_through += 1;
            } else {
                break;
            }
        }
        for segment in self.sealed.drain(..delete_through) {
            if let Err(error) = fs::remove_file(&segment.path) {
                tracing::warn!(?error, path = ?segment.path, "failed to remove sealed wal segment");
            }
        }
    }

    fn rotate(&mut self) -> Option<PathBuf> {
        let held = std::mem::replace(
            &mut self.wal,
            Err(EngineError::IoError(std::io::Error::other("wal rotating"))),
        );

        let mut writer = match held {
            Ok(writer) => writer,
            Err(error) => {
                tracing::error!(?error, "cannot seal wal segment: writer is not open");
                return None;
            }
        };

        if let Err(error) = writer.sync_data() {
            // The BufWriter on drop still pushes buffered bytes to the OS, so
            // dropping and renaming preserves every appended record.
            tracing::error!(?error, "failed to sync wal before sealing");
        }
        drop(writer);

        let sealed = sealed_path(&self.active, self.next_seq);
        self.next_seq += 1;

        if let Err(error) = fs::rename(&self.active, &sealed) {
            tracing::error!(?error, ?sealed, "failed to seal wal segment");
            self.wal = WalWriter::open(&self.active);
            return None;
        }
        self.wal = WalWriter::open(&self.active);
        Some(sealed)
    }
}

#[must_use]
pub fn spawn_wal_worker(
    config: Arc<EngineConfig>,
    receiver: WalRx<WalCommand>,
    truncate_rx: WalRx<usize>,
    memtable_tx: HandoffTx<Vec<LogRecord>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let wal_path = config.wal_path.clone();
        run_wal_loop(&wal_path, receiver, truncate_rx, &memtable_tx);
    })
}

#[must_use]
pub fn spawn_memtable_worker(
    receiver: HandoffRx<Vec<LogRecord>>,
    truncate_tx: WalTx<usize>,
    flusher: ParquetFlusher,
    live_buffer: LiveBuffer,
    config: Arc<EngineConfig>,
) -> JoinHandle<()> {
    spawn_memtable_worker_with_broadcast(receiver, truncate_tx, flusher, live_buffer, config, None)
}

#[must_use]
pub fn spawn_memtable_worker_with_broadcast(
    receiver: HandoffRx<Vec<LogRecord>>,
    truncate_tx: WalTx<usize>,
    flusher: ParquetFlusher,
    live_buffer: LiveBuffer,
    config: Arc<EngineConfig>,
    broadcast_tx: Option<broadcast::Sender<Vec<LogRecord>>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        run_memtable_loop(&receiver, &truncate_tx, &flusher, &live_buffer, &config, broadcast_tx);
    })
}

fn run_wal_loop(
    wal_path: &Path,
    mut receiver: WalRx<WalCommand>,
    truncate_rx: WalRx<usize>,
    memtable_tx: &HandoffTx<Vec<LogRecord>>,
) {
    let mut wal = WalState::open(wal_path.to_path_buf());
    match tokio::runtime::Builder::new_current_thread().build() {
        Ok(runtime) => {
            runtime.block_on(select_commands(&mut wal, &mut receiver, truncate_rx, memtable_tx));
        }
        Err(error) => {
            tracing::error!(?error, "failed to build wal worker runtime; falling back to blocking drain");
            drain_channel(&mut wal.wal, &mut receiver, memtable_tx);
        }
    }
}

async fn select_commands(
    wal: &mut WalState,
    receiver: &mut WalRx<WalCommand>,
    truncate_rx: WalRx<usize>,
    memtable_tx: &HandoffTx<Vec<LogRecord>>,
) {
    let mut truncate_rx = Some(truncate_rx);
    loop {
        tokio::select! {
            command = receiver.recv() => match command {
                Some(WalCommand::Append(batch)) => {
                    handle_append_group(wal, batch, receiver, memtable_tx);
                },
                None => break,
            },
            signal = truncate_signal(&mut truncate_rx) => match signal {
                Some(rows) => wal.confirm(rows),
                None => truncate_rx = None,
            },
        }
    }
}

async fn truncate_signal(rx: &mut Option<WalRx<usize>>) -> Option<usize> {
    match rx {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

fn drain_channel(
    wal: &mut Result<WalWriter, EngineError>,
    receiver: &mut WalRx<WalCommand>,
    memtable_tx: &HandoffTx<Vec<LogRecord>>,
) {
    while let Some(command) = receiver.blocking_recv() {
        match command {
            WalCommand::Append(batch) => handle_append_single(wal, batch, memtable_tx),
        }
    }
}

/// Group commit: coalesces up to `MAX_GROUP_BATCH` appends into a single fsync.
fn handle_append_group(
    wal: &mut WalState,
    first: IngestBatch,
    receiver: &mut WalRx<WalCommand>,
    memtable_tx: &HandoffTx<Vec<LogRecord>>,
) {
    let mut batches: Vec<IngestBatch> = Vec::new();
    batches.push(first);

    for _ in 1..MAX_GROUP_BATCH {
        match receiver.try_recv() {
            Ok(WalCommand::Append(batch)) => batches.push(batch),
            Err(
                tokio::sync::mpsc::error::TryRecvError::Empty
                | tokio::sync::mpsc::error::TryRecvError::Disconnected,
            ) => break,
        }
    }

    let outcome: Result<(), EngineError> = match &mut wal.wal {
        Ok(writer) => {
            let mut result: Result<(), EngineError> = Ok(());
            for batch in &batches {
                if let Err(e) = writer.append_batch_no_sync(&batch.records) {
                    result = Err(e);
                    break;
                }
            }
            if result.is_ok() {
                if let Err(e) = writer.sync_data() {
                    result = Err(e);
                }
            }
            result
        }
        Err(original) => Err(fallback_error(original)),
    };

    let is_ok = outcome.is_ok();
    // One display string shared by every failing responder.
    let error_string = match &outcome {
        Err(e) => e.to_string(),
        Ok(()) => String::new(),
    };

    let mut written = 0usize;
    for batch in batches {
        let responder = batch.responder;
        let records = batch.records;
        if is_ok {
            written += records.len();
            // Receiver gone means client gone; the records are durable.
            let _ = responder.send(Ok(()));
            if let Err(e) = memtable_tx.send(records) {
                tracing::error!(?e, "handoff channel dropped or full; losing durable records - triggering shutdown may be required");
            }
        } else {
            let err = EngineError::IoError(std::io::Error::other(error_string.clone()));
            let _ = responder.send(Err(err));
        }
    }
    wal.records_written(written);
}

fn handle_append_single(
    wal: &mut Result<WalWriter, EngineError>,
    batch: IngestBatch,
    memtable_tx: &HandoffTx<Vec<LogRecord>>,
) {
    let outcome = match wal {
        Ok(writer) => writer.append_batch(&batch.records),
        Err(error) => Err(fallback_error(error)),
    };
    let durable = outcome.is_ok();
    let _ = batch.responder.send(outcome);
    if durable {
        if let Err(e) = memtable_tx.send(batch.records) {
            tracing::error!(?e, "handoff channel dropped or full; losing durable records");
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn run_memtable_loop(
    receiver: &HandoffRx<Vec<LogRecord>>,
    truncate_tx: &WalTx<usize>,
    flusher: &ParquetFlusher,
    live_buffer: &LiveBuffer,
    config: &EngineConfig,
    broadcast_tx: Option<broadcast::Sender<Vec<LogRecord>>>,
) {
    let limit = config.flush_row_limit;
    let flush_interval = Duration::from_secs(config.flush_interval_secs.max(1));
    // Measured from startup; reset by every flush attempt.
    let mut last_flush = std::time::Instant::now();
    let mut table = MemTable::new(limit);
    loop {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(records) => {
                if let Some(tx) = &broadcast_tx {
                    let _ = tx.send(records.clone());
                }
                for record in &records {
                    table.append_record(record);
                }
                if table.num_rows() >= limit {
                    finish_and_push(live_buffer, &mut table, "threshold");
                }
            }
            Err(crossbeam::channel::RecvTimeoutError::Timeout) => {
                if !table.is_empty() {
                    finish_and_push(live_buffer, &mut table, "timeout");
                }
            }
            Err(crossbeam::channel::RecvTimeoutError::Disconnected) => break,
        }
        let interval_due = last_flush.elapsed() >= flush_interval;
        if interval_due && !table.is_empty() {
            finish_and_push(live_buffer, &mut table, "interval");
        }
        let buffered = live_row_count(live_buffer);
        if (buffered >= limit || interval_due) && buffered > 0 {
            flush_live_buffer(live_buffer, truncate_tx, flusher);
            // Reset the clock whether or not the flush succeeded. Returned
            // batches leave the buffer over the limit, so without this a retry
            // would fire on every tick and hammer a failing disk instead of
            // backing off to the configured interval.
            last_flush = std::time::Instant::now();
        }
    }
    if !table.is_empty() {
        finish_and_push(live_buffer, &mut table, "remainder");
    }
    if live_row_count(live_buffer) > 0 {
        flush_live_buffer(live_buffer, truncate_tx, flusher);
    }
}

fn finish_and_push(live_buffer: &LiveBuffer, table: &mut MemTable, reason: &str) {
    match table.finish() {
        Ok(batch) => push_to_live_buffer(live_buffer, batch),
        Err(error) => tracing::error!(
            ?error,
            ?reason,
            "failed to build columnar batch; rows stay in the WAL and replay on restart"
        ),
    }
}

fn push_to_live_buffer(live_buffer: &LiveBuffer, batch: RecordBatch) {
    if batch.num_rows() == 0 {
        return;
    }
    if let Ok(mut buffer) = live_buffer.write() {
        buffer.push(batch);
    } else {
        tracing::error!("live buffer write lock poisoned; dropping batch");
    }
}

fn live_row_count(live_buffer: &LiveBuffer) -> usize {
    if let Ok(buffer) = live_buffer.read() {
        buffer.iter().map(RecordBatch::num_rows).sum()
    } else {
        tracing::error!("live buffer read lock poisoned; assuming empty");
        0
    }
}

fn flush_live_buffer(
    live_buffer: &LiveBuffer,
    truncate_tx: &WalTx<usize>,
    flusher: &ParquetFlusher,
) {
    let batches = if let Ok(mut buffer) = live_buffer.write() {
        std::mem::take(&mut *buffer)
    } else {
        tracing::error!("live buffer write lock poisoned; skipping flush");
        return;
    };

    let concatenated = match arrow::compute::concat_batches(&greplog_schema(), &batches) {
        Ok(concatenated) => concatenated,
        Err(error) => {
            tracing::error!(?error, "failed to concatenate live batches; returning them to the buffer");
            return_to_live_buffer(live_buffer, batches);
            return;
        }
    };

    let rows = concatenated.num_rows();
    if let Err(error) = flusher.flush(&concatenated) {
        tracing::error!(?error, rows, "failed to write parquet; returning rows to the buffer for retry");
        return_to_live_buffer(live_buffer, batches);
        return;
    }

    tracing::info!("Flushed batch of {rows} rows");
    let _ = truncate_tx.blocking_send(rows);
}

fn return_to_live_buffer(live_buffer: &LiveBuffer, mut batches: Vec<RecordBatch>) {
    let Ok(mut buffer) = live_buffer.write() else {
        tracing::error!(
            batches = batches.len(),
            "live buffer write lock poisoned; unflushed rows stay in the WAL and replay on restart"
        );
        return;
    };
    batches.append(&mut buffer);
    *buffer = batches;
}

/// Reconstructs an ingest-visible error when the WAL never opened.
/// `EngineError` is deliberately not `Clone`, so every in-flight batch gets a
/// fresh `io::Error` carrying the original message.
fn fallback_error(original: &EngineError) -> EngineError {
    EngineError::IoError(std::io::Error::other(original.to_string()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tempfile::tempdir;
    use tokio::sync::{mpsc, oneshot};

    use super::{spawn_memtable_worker, spawn_wal_worker};
    use crate::config::{EngineConfig, LiveBuffer};
    use crate::ingest::{IngestBatch, WalCommand};
    use crate::record::LogRecord;
    use crate::storage::ParquetFlusher;

    fn sample(index: usize) -> LogRecord {
        LogRecord::new("INFO", "auth-api", format!("boot {index}"))
    }

    fn test_config(dir: &tempfile::TempDir) -> Arc<EngineConfig> {
        Arc::new(EngineConfig {
            data_dir: dir.path().join("logs"),
            wal_path: dir.path().join("current.wal"),
            mpsc_buffer_size: 4,
            crossbeam_buffer_size: 4,
            ..EngineConfig::default()
        })
    }

    #[test]
    fn wal_worker_persists_and_confirms_batch() {
        let runtime = tokio::runtime::Runtime::new().expect("build runtime");
        runtime.block_on(async {
            let dir = tempdir().expect("create temp dir");
            let config = test_config(&dir);
            let (wal_tx, wal_rx) = mpsc::channel(config.mpsc_buffer_size);
            let (truncate_tx, truncate_rx) = mpsc::channel(config.mpsc_buffer_size);
            let (handoff_tx, _handoff_rx) =
                crossbeam::channel::bounded(config.crossbeam_buffer_size);
            let handle = spawn_wal_worker(config, wal_rx, truncate_rx, handoff_tx);

            let (responder, ack) = oneshot::channel();
            let batch = IngestBatch::new(vec![sample(1)], responder);
            wal_tx.send(WalCommand::Append(batch)).await.expect("send append");
            drop(wal_tx);
            drop(truncate_tx);

            let acked = ack.await.expect("worker must respond");
            assert!(acked.is_ok(), "wal append must succeed");

            handle.join().expect("worker thread must exit cleanly");
        });
    }

    #[test]
    fn wal_worker_answers_error_without_panicking() {
        let runtime = tokio::runtime::Runtime::new().expect("build runtime");
        runtime.block_on(async {
            let dir = tempdir().expect("create temp dir");
            let config = Arc::new(EngineConfig {
                wal_path: dir.path().join("missing-dir").join("current.wal"),
                ..EngineConfig::default()
            });
            let (wal_tx, wal_rx) = mpsc::channel(config.mpsc_buffer_size);
            let (truncate_tx, truncate_rx) = mpsc::channel(config.mpsc_buffer_size);
            let (handoff_tx, _handoff_rx) =
                crossbeam::channel::bounded(config.crossbeam_buffer_size);
            let handle = spawn_wal_worker(config, wal_rx, truncate_rx, handoff_tx);

            let batch = IngestBatch {
                records: Vec::new(),
                responder: oneshot::channel().0,
            };
            wal_tx.send(WalCommand::Append(batch)).await.expect("send append");
            drop(wal_tx);
            drop(truncate_tx);

            handle.join().expect("worker thread must exit cleanly");
        });
    }

    #[test]
    fn memtable_worker_drains_record_stream() {
        let dir = tempdir().expect("create temp dir");
        let config = test_config(&dir);
        let (truncate_tx, truncate_rx) = mpsc::channel(config.mpsc_buffer_size);
        drop(truncate_rx);
        let flusher = ParquetFlusher::new(&config);
        let (sender, receiver) = crossbeam::channel::bounded(config.crossbeam_buffer_size);
        let handle = spawn_memtable_worker(receiver, truncate_tx, flusher, LiveBuffer::default(), config);

        for index in 0..64 {
            sender.send(vec![sample(index)]).expect("send record stream");
        }
        drop(sender);

        handle.join().expect("memtable worker must exit cleanly");
    }

    #[test]
    fn memtable_worker_flushes_on_interval_below_limit() {
        let dir = tempdir().expect("create temp dir");
        let config = Arc::new(EngineConfig {
            data_dir: dir.path().join("logs"),
            wal_path: dir.path().join("current.wal"),
            flush_row_limit: 10_000,
            flush_interval_secs: 1,
            mpsc_buffer_size: 4,
            crossbeam_buffer_size: 8,
            ..EngineConfig::default()
        });
        let (truncate_tx, truncate_rx) = mpsc::channel(config.mpsc_buffer_size);
        drop(truncate_rx);
        let flusher = ParquetFlusher::new(&config);
        let (sender, receiver) = crossbeam::channel::bounded(config.crossbeam_buffer_size);
        let handle =
            spawn_memtable_worker(receiver, truncate_tx, flusher, LiveBuffer::default(), config);

        for index in 0..8 {
            sender.send(vec![sample(index)]).expect("send record stream");
        }

        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let has_partition = std::fs::read_dir(dir.path().join("logs"))
                .is_ok_and(|entries| entries.count() > 0);
            if has_partition {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "parquet must be written on the periodic interval, well below the row limit"
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        drop(sender);
        handle.join().expect("memtable worker must exit cleanly");
    }

    #[test]
    fn memtable_worker_flushes_at_the_configured_limit() {
        let dir = tempdir().expect("create temp dir");
        let config = Arc::new(EngineConfig {
            data_dir: dir.path().join("logs"),
            wal_path: dir.path().join("current.wal"),
            flush_row_limit: 16,
            mpsc_buffer_size: 4,
            crossbeam_buffer_size: 8,
            ..EngineConfig::default()
        });
        let (truncate_tx, truncate_rx) = mpsc::channel(config.mpsc_buffer_size);
        drop(truncate_rx);
        let flusher = ParquetFlusher::new(&config);
        let (sender, receiver) = crossbeam::channel::bounded(config.crossbeam_buffer_size);
        let handle = spawn_memtable_worker(receiver, truncate_tx, flusher, LiveBuffer::default(), config);

        for index in 0..32 {
            sender.send(vec![sample(index)]).expect("send record stream");
        }
        drop(sender);

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let has_partition = std::fs::read_dir(dir.path().join("logs"))
                .is_ok_and(|entries| entries.count() > 0);
            if has_partition {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "parquet must be written");
            std::thread::sleep(Duration::from_millis(10));
        }

        handle.join().expect("memtable worker must exit cleanly");
    }
}
