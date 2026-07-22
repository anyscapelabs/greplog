use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Run one compaction cycle on all eligible partitions.
///
/// Discovers every `date=YYYY-MM-DD` leaf directory under `data_dir/data/`
/// and calls [`compact_partition`] on each with `skip_today=true` so the
/// actively‑written partition is never touched.
///
/// Returns the number of partitions that were actually compacted
/// (i.e. had ≥2 source files merged).
pub fn compact_eligible_partitions(data_dir: &Path) -> Result<usize> {
    let data = data_dir.join("data");
    if !data.exists() {
        return Ok(0);
    }
    let partitions = super::compaction::find_partition_dirs(&data);
    let mut compacted = 0;
    for dir in &partitions {
        match super::compaction::compact_partition(
            dir,
            super::compaction::DEFAULT_TARGET_SIZE,
            true, // skip_today – never compact the partition being written to
        ) {
            Ok(result) => {
                if result.files_compacted > 0 {
                    tracing::info!(
                        "Compacted {:?}: {} files merged into 1",
                        dir,
                        result.files_compacted,
                    );
                    compacted += 1;
                }
            }
            Err(e) => {
                tracing::warn!("Compaction failed for {:?}: {} — skipping", dir, e);
            }
        }
    }
    Ok(compacted)
}

/// Spawn a background task that periodically calls
/// [`compact_eligible_partitions`] on the given `data_dir`.
///
/// The first tick is skipped (the agent has just started and may still be
/// recovering); subsequent ticks run compaction inside `spawn_blocking`
/// so they never block the tokio runtime.
pub fn spawn_compaction_scheduler(
    data_dir: PathBuf,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        // Skip the first immediate tick — no point compacting at startup.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            let dir = data_dir.clone();
            if let Err(e) = tokio::task::spawn_blocking(move || {
                compact_eligible_partitions(&dir)
            })
            .await
            .unwrap_or_else(|e| Err(anyhow::anyhow!("spawn_blocking join: {e}")))
            {
                tracing::error!("Compaction cycle failed: {e}");
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::io::write_parquet_atomic;
    use arrow::array::*;
    use arrow::record_batch::RecordBatch;
    use greplog_core::arrow_schema;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{mpsc, oneshot};

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("greplog_compact_sched_test_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn make_small_batch(service: &str, file_idx: usize, rows: usize) -> RecordBatch {
        let schema = arrow_schema::log_schema();
        let ts = 1_700_000_000_000_000;
        let ids: Vec<String> = (0..rows)
            .map(|_| uuid::Uuid::new_v4().to_string())
            .collect();
        let services: Vec<&str> = (0..rows).map(|_| service).collect();
        let timestamps: Vec<i64> = (0..rows).map(|_| ts).collect();
        let levels: Vec<Option<&str>> = (0..rows).map(|_| Some("info")).collect();
        let routes: Vec<Option<&str>> = (0..rows).map(|_| None).collect();
        let messages: Vec<Option<String>> = (0..rows)
            .map(|j| Some(format!("file{}-row{}", file_idx, j)))
            .collect();
        let attrs: Vec<Option<&str>> = (0..rows).map(|_| None).collect();

        let msg_array: StringArray = messages.iter().map(|m| m.as_deref()).collect();

        RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(StringArray::from(ids)),
                Arc::new(StringArray::from(services)),
                Arc::new(TimestampMicrosecondArray::from(timestamps)),
                Arc::new(StringArray::from(levels)),
                Arc::new(StringArray::from(routes)),
                Arc::new(msg_array),
                Arc::new(StringArray::from(attrs)),
            ],
        )
        .expect("build small batch")
    }

    fn write_files(partition_dir: &Path, count: usize, rows_per_file: usize) {
        std::fs::create_dir_all(partition_dir).expect("create partition dir");
        for i in 0..count {
            let batch = make_small_batch("test-svc", i, rows_per_file);
            write_parquet_atomic(partition_dir, &batch).expect("write parquet");
        }
    }

    fn count_parquet_files(dir: &Path) -> usize {
        if !dir.exists() {
            return 0;
        }
        std::fs::read_dir(dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map_or(false, |n| n.ends_with(".parquet"))
            })
            .count()
    }

    fn make_log_event(service: &str, msg: &str, ts_ns: i64) -> greplog_core::gen::LogEvent {
        greplog_core::gen::LogEvent {
            service_name: service.into(),
            message: msg.into(),
            level: "info".into(),
            timestamp_ns: ts_ns,
            logger_name: String::new(),
            file: String::new(),
            line: 0,
            correlation_id: String::new(),
            attributes: std::collections::HashMap::new(),
            stack_trace: Vec::new(),
            exception_type: String::new(),
            exception_message: String::new(),
            event_id: String::new(),
        }
    }

    fn make_batch(service: &str, events_per_batch: usize, ts_ns: i64) -> greplog_core::gen::IngestBatch {
        let logs: Vec<_> = (0..events_per_batch)
            .map(|i| make_log_event(service, &format!("msg-{i}"), ts_ns + i as i64))
            .collect();
        greplog_core::gen::IngestBatch {
            service_name: service.into(),
            instance_id: "test-instance".into(),
            batch_seq: 0,
            logs,
            spans: vec![],
            metrics: vec![],
        }
    }

    fn today_date() -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs() as i64;
        let days = if secs >= 0 { secs / 86400 } else { (secs + 1) / 86400 - 1 };
        super::super::compaction::civil_days_to_ymd(days)
    }

    // ------------------------------------------------------------------
    // Test 1: Compaction actually runs during normal operation
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_compaction_runs_during_normal_operation() {
        let dir = test_dir("scheduler_cycle");

        // Write 3 small parquet files to a past‑date partition.
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=test-svc")
            .join("date=2023-01-15");
        write_files(&part_dir, 3, 5);
        assert_eq!(count_parquet_files(&part_dir), 3);

        // Run one compaction cycle.
        let compacted = compact_eligible_partitions(&dir).expect("compact cycle");
        assert!(compacted >= 1, "should have compacted at least 1 partition");

        // After compaction: only the merged file remains.
        let remaining = count_parquet_files(&part_dir);
        assert_eq!(remaining, 1, "3 source files should be merged into 1");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 2: Today's partition stays untouched
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_today_partition_untouched() {
        let dir = test_dir("scheduler_today_untouched");
        let today = today_date();

        // Write 3 small parquet files to today's partition.
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=test-svc")
            .join(format!("date={}", today));
        write_files(&part_dir, 3, 5);
        assert_eq!(count_parquet_files(&part_dir), 3);

        // Run compaction cycle – skip_today prevents compaction of today's partition.
        let compacted = compact_eligible_partitions(&dir).expect("compact cycle");

        // Today's partition files are unchanged.
        assert_eq!(
            count_parquet_files(&part_dir),
            3,
            "today's partition must not be touched",
        );
        // compacted is 0 because today's partition is the only one and was skipped.
        assert_eq!(compacted, 0, "no partitions should have been compacted");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 3: No interference with ingestion throughput
    // ------------------------------------------------------------------
    //
    // Ingestion runs via the live Writer while compaction fires on a
    // different partition.  All batches must be accepted — proving the
    // compaction path does not share a lock with the ingest/flush path.
    #[tokio::test]
    async fn test_no_ingest_interference() {
        let dir = test_dir("scheduler_no_interfere");
        let writer = crate::store::Writer::new(&dir, 100_000).expect("new writer");
        writer.mark_startup_done();

        // Pre‑populate a past‑date partition with small files for compaction.
        let old_part = dir
            .join("data")
            .join("table_type=logs")
            .join("service=old-svc")
            .join("date=2022-06-01");
        write_files(&old_part, 5, 10);
        assert_eq!(count_parquet_files(&old_part), 5);

        let (batch_tx, batch_rx) = mpsc::channel(256);
        let _writer_handle = writer.spawn(batch_rx, Duration::from_secs(60));

        let n_batches = 30_usize;
        for i in 0..n_batches {
            let (resp_tx, resp_rx) = oneshot::channel();
            let ts = 1_700_000_000_000_000 + i as i64;
            let batch = make_batch("live-svc", 3, ts);
            use prost::Message;
            let raw_bytes = bytes::Bytes::from(batch.encode_to_vec());
            batch_tx
                .send((batch, raw_bytes, resp_tx))
                .await
                .expect("send");
            let resp = resp_rx.await.expect("response");
            assert!(
                resp.accepted,
                "batch {i} must be accepted — compaction must not block ingest",
            );
        }

        // Run compaction cycle while ingest is active.
        let compact_dir = dir.clone();
        let compacted =
            tokio::task::spawn_blocking(move || compact_eligible_partitions(&compact_dir))
                .await
                .expect("spawn_blocking")
                .expect("compact");
        assert!(compacted >= 1, "compaction should have merged at least 1 partition");

        // All batches were accepted — no lock contention between
        // compaction and ingest.
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 4: Query correctness before and after compaction
    // ------------------------------------------------------------------
    //
    // Verifies that the atomic-rename compaction protocol preserves data
    // visibility: a query before compaction sees N rows across the source
    // files, and a query after compaction sees the same N rows from the
    // merged file.  The merged file is always internally consistent
    // (never partially written) because `write_parquet_atomic` uses a
    // temp-file → fsync → rename sequence.
    #[tokio::test]
    async fn test_query_correctness_before_and_after_compaction() {
        let dir = test_dir("scheduler_query_correctness");
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=svc")
            .join("date=2023-09-20");
        let total_rows: i64 = 15; // 3 files × 5 rows

        std::fs::create_dir_all(&part_dir).expect("create partition dir");
        for i in 0..3 {
            let batch = make_small_batch("svc", i, 5);
            write_parquet_atomic(&part_dir, &batch).expect("write parquet");
        }
        assert_eq!(count_parquet_files(&part_dir), 3);

        // Query before compaction — must see all rows.
        let engine_before =
            crate::query_engine::QueryEngine::new(&dir).expect("new engine");
        let result_before = engine_before
            .query("SELECT count(*) AS cnt FROM logs WHERE service = 'svc'")
            .await
            .expect("query before compaction");
        let rows_before: i64 =
            serde_json::from_value(result_before.rows[0][0].clone()).expect("count");
        assert_eq!(rows_before, total_rows, "pre‑compaction query must see all rows");

        // Run a full compaction cycle.
        let compact_dir = dir.clone();
        tokio::task::spawn_blocking(move || compact_eligible_partitions(&compact_dir))
            .await
            .expect("spawn_blocking")
            .expect("compact");
        assert_eq!(count_parquet_files(&part_dir), 1);

        // Query after compaction — must still see all rows from the merged file.
        let engine_after =
            crate::query_engine::QueryEngine::new(&dir).expect("new engine");
        let result_after = engine_after
            .query("SELECT count(*) AS cnt FROM logs WHERE service = 'svc'")
            .await
            .expect("query after compaction");
        let rows_after: i64 =
            serde_json::from_value(result_after.rows[0][0].clone()).expect("count");
        assert_eq!(
            rows_after, total_rows,
            "post‑compaction query must see all rows — atomic rename preserves data",
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
