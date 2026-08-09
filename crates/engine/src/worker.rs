//! The dedicated OS worker threads of the write path.
//!
//! Disk I/O and columnar conversion never run on the Tokio async runtime:
//! - [`spawn_wal_worker`] persists batches to `current.wal` and `fsync`s.
//! - [`spawn_memtable_worker`] converts durable records into Arrow batches.
//!
//! The two are joined by a lock-free `crossbeam` channel, so the `MemTable` only
//! ever sees records that are already safe on disk.

use std::path::PathBuf;
use std::thread::{self, JoinHandle};

use crossbeam::channel::{Receiver as HandoffRx, Sender as HandoffTx};
use tokio::sync::mpsc::Receiver as IngestRx;

use crate::ingest::IngestBatch;
use crate::memtable::{MemTable, FLUSH_THRESHOLD};
use crate::record::LogRecord;
use crate::wal::WalWriter;

/// Spawns the WAL worker on a dedicated OS thread and returns its handle.
///
/// Durable batches are forwarded over `memtable_tx` so the `MemTable` worker
/// receives records only after they are `fsync`ed.
///
/// The loop ends when the channel closes (all senders dropped); call
/// [`JoinHandle::join`] to wait for outstanding `fsync`s to finish.
#[must_use]
pub fn spawn_wal_worker(
    wal_path: PathBuf,
    mut receiver: IngestRx<IngestBatch>,
    memtable_tx: HandoffTx<Vec<LogRecord>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let wal = WalWriter::open(&wal_path);
        drain_channel(wal, &mut receiver, &memtable_tx);
    })
}

/// Spawns the `MemTable` worker on a dedicated OS thread and returns its handle.
///
/// Records are appended into an in-memory Arrow [`MemTable`]; once
/// [`FLUSH_THRESHOLD`] rows accumulate the batch is finished (for now logged
/// and discarded — Parquet writing arrives in a later round).
#[must_use]
pub fn spawn_memtable_worker(receiver: HandoffRx<Vec<LogRecord>>) -> JoinHandle<()> {
    thread::spawn(move || run_memtable_loop(&receiver))
}

/// Drains batches off the ingest channel, persisting each before acknowledging.
fn drain_channel(
    mut wal: Result<WalWriter, crate::error::EngineError>,
    receiver: &mut IngestRx<IngestBatch>,
    memtable_tx: &HandoffTx<Vec<LogRecord>>,
) {
    while let Some(batch) = receiver.blocking_recv() {
        let outcome = match &mut wal {
            Ok(writer) => writer.append_batch(&batch.records),
            Err(error) => Err(fallback_error(error)),
        };
        let durable = outcome.is_ok();
        let _ = batch.responder.send(outcome);
        if durable {
            let _ = memtable_tx.send(batch.records);
        }
    }
}

/// Drains durable record streams into columnar batches, flushing on threshold.
fn run_memtable_loop(receiver: &HandoffRx<Vec<LogRecord>>) {
    let mut table = MemTable::new();
    let mut rows = 0usize;
    while let Ok(records) = receiver.recv() {
        for record in records {
            table.append_record(&record);
            rows += 1;
        }
        if rows >= FLUSH_THRESHOLD {
            flush(&mut table, rows);
            rows = 0;
        }
    }
    if rows > 0 {
        flush(&mut table, rows);
    }
}

/// Finishes the current memtable batch and logs its size.
fn flush(table: &mut MemTable, rows: usize) {
    match table.finish() {
        Ok(batch) => tracing::info!("Flushed batch of {} rows", batch.num_rows()),
        Err(error) => tracing::error!(?error, "failed to flush memtable with {rows} rows"),
    }
}

/// Reconstructs an ingest-visible error when the WAL never opened.
///
/// `EngineError` is deliberately not `Clone` (its payloads cannot cheaply
/// reproduce it), so when the WAL fails to open we report a fresh [`std::io::Error`]
/// carrying the original message to every in-flight batch.
fn fallback_error(original: &crate::error::EngineError) -> crate::error::EngineError {
    crate::error::EngineError::IoError(std::io::Error::other(original.to_string()))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use tokio::sync::{mpsc, oneshot};

    use super::{spawn_memtable_worker, spawn_wal_worker};
    use crate::ingest::IngestBatch;
    use crate::record::LogRecord;

    fn sample(index: usize) -> LogRecord {
        LogRecord::new("INFO", "auth-api", format!("boot {index}"))
    }

    #[test]
    fn wal_worker_persists_and_confirms_batch() {
        let runtime = tokio::runtime::Runtime::new().expect("build runtime");
        runtime.block_on(async {
            let dir = tempdir().expect("create temp dir");
            let path = dir.path().join("current.wal");
            let (sender, receiver) = mpsc::channel(4);
            let (handoff_tx, _handoff_rx) = crossbeam::channel::unbounded();
            let handle = spawn_wal_worker(path, receiver, handoff_tx);

            let (responder, ack) = oneshot::channel();
            let batch = IngestBatch::new(vec![sample(1)], responder);
            sender.send(batch).await.expect("send batch");
            drop(sender);

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
            let path = dir.path().join("current.wal");
            let (sender, receiver) = mpsc::channel(4);
            let (handoff_tx, _handoff_rx) = crossbeam::channel::unbounded();
            let handle = spawn_wal_worker(path, receiver, handoff_tx);

            let batch = IngestBatch {
                records: Vec::new(),
                responder: oneshot::channel().0,
            };
            sender.send(batch).await.expect("send batch");
            drop(sender);

            handle.join().expect("worker thread must exit cleanly");
        });
    }

    #[test]
    fn memtable_worker_drains_record_stream() {
        let (sender, receiver) = crossbeam::channel::unbounded();
        let handle = spawn_memtable_worker(receiver);

        for index in 0..64 {
            sender.send(vec![sample(index)]).expect("send record stream");
        }
        drop(sender);

        handle.join().expect("memtable worker must exit cleanly");
    }
}