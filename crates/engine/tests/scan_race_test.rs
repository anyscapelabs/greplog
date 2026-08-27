//! Regression tests for the split-file-write visibility invariant.
//!
//! Chunk files are written to a `<name>.parquet.part` temp name and atomically
//! renamed to `<name>.parquet` only once fully written and closed. DataFusion's
//! [`ListingTable`] globs `**/*.parquet`, so any temp file whose name ends in
//! `.parquet` would be discovered mid-write and fail to parse, surfacing as an
//! intermittent scan `internal error` under concurrent flushes. These tests pin
//! that `.part` files are never scanned.

use std::sync::Arc;

use greplog_engine::config::{EngineConfig, LiveBuffer};
use greplog_engine::memtable::MemTable;
use greplog_engine::query::QueryEngine;
use greplog_engine::record::LogRecord;
use greplog_engine::storage::ParquetFlusher;
use tempfile::tempdir;

/// Builds the day-partition -- `data_dir/year=YYYY/month=MM/day=DD` -- the
/// same way [`ParquetFlusher`] does, so tests can plant files next to chunks.
fn date_dir(data_dir: &std::path::Path) -> std::path::PathBuf {
    let now = chrono::Utc::now();
    data_dir
        .join(format!("year={}", now.format("%Y")))
        .join(format!("month={}", now.format("%m")))
        .join(format!("day={}", now.format("%d")))
}

fn record(index: usize, message: &str) -> LogRecord {
    LogRecord {
        timestamp_us: 1_700_000_000_000_000 + i64::try_from(index).expect("fit i64"),
        trace_id: None,
        span_id: None,
        parent_span_id: None,
        duration_ms: None,
        level: if index.is_multiple_of(3) {
            "ERROR".to_string()
        } else {
            "INFO".to_string()
        },
        service: "orders".to_string(),
        message: message.to_string(),
        raw_body: None,
    }
}

fn flush_flusher(config: &EngineConfig, mut table: MemTable) {
    let batch = table.finish().expect("finish memtable");
    ParquetFlusher::new(config)
        .flush(&batch)
        .expect("flush parquet");
}

/// The listing table must completely ignore a leftover `.parquet.part` file,
/// even one containing garbage, and only scan chunks that were atomically
/// renamed into place.
#[tokio::test]
async fn trailing_part_files_are_never_scanned() {
    let dir = tempdir().expect("create temp dir");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };

    let engine = QueryEngine::new(&config, LiveBuffer::default())
        .await
        .expect("build query engine");

    // A real, durable chunk of 100 rows whose messages match the predicate.
    let mut table = MemTable::new(100);
    for index in 0..100 {
        table.append_record(&record(index, "payment failed"));
    }
    flush_flusher(&config, table);

    // A partial write that crashed before its rename: half-written parquet
    // bytes under a `.parquet.part` name in the same partition directory.
    let day = date_dir(&config.data_dir);
    let partial = day.join("orders").join("chunk_1.parquet.part");
    std::fs::create_dir_all(
        partial
            .parent()
            .expect("partition dir exists for partial file"),
    )
    .expect("create service dir");
    std::fs::write(&partial, b"PAR1not really a complete parquet file")
        .expect("write partial file");

    let batches = engine
        .execute_sql("SELECT count(*) FROM logs WHERE message ILIKE '%failed%'")
        .await
        .expect("query must skip .parquet.part files, not fail on them");

    let batch = batches.first().expect("count query produces a batch");
    let count = batch
        .column(0)
        .as_any()
        .downcast_ref::<arrow::array::Int64Array>()
        .expect("count(*) is an Int64 column")
        .value(0);

    assert_eq!(count, 100, "only the complete chunk may contribute rows");
    assert!(
        partial.exists(),
        "the partial file is left on disk for the next flush's cleanup"
    );
}

/// Hardens the read path against concurrent flushes: scans (ILIKE + group-by,
/// which cannot prune any partition) run uninterrupted while several threads
/// are continuously writing and atomically renaming chunk files. A scan that
/// observes a half-written file would error out of this loop.
#[tokio::test]
async fn concurrent_flushes_never_break_scans() {
    let dir = tempdir().expect("create temp dir");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };

    let engine = Arc::new(
        QueryEngine::new(&config, LiveBuffer::default())
            .await
            .expect("build query engine"),
    );

    const WRITES: usize = 12;
    const QUERIES: usize = 40;

    let writers: Vec<_> = (0..2)
        .map(|writer_id| {
            let config = config.clone();
            std::thread::spawn(move || {
                for round in 0..WRITES {
                    let mut table = MemTable::new(900);
                    for index in 0..900 {
                        let message = match (round + writer_id + index) % 4 {
                            0 => "payment failed",
                            1 => "card declined",
                            2 => "checkout started",
                            _ => "retry scheduled",
                        };
                        table.append_record(&record(index, message));
                    }
                    flush_flusher(&config, table);
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().expect("writer thread finishes");
    }

    let scanner = |engine: Arc<QueryEngine>| async move {
        for _ in 0..QUERIES {
            engine
                .execute_sql(
                    "SELECT level, count(*) FROM logs \
                     WHERE message ILIKE '%failed%' OR message ILIKE '%declined%' \
                     GROUP BY level",
                )
                .await
                .expect("scan must never fail while chunks are being written");
        }
    };

    tokio::join!(scanner(Arc::clone(&engine)), scanner(Arc::clone(&engine)));
}
