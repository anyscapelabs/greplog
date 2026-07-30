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

    tokio::time::sleep(Duration::from_millis(250)).await;

    let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
    let result = engine.query("SELECT count(*) AS cnt FROM logs").await.expect("query");
    assert_eq!(result.rows[0][0], serde_json::json!(5i64), "5 events flushed");

    assert_eq!(dropped.load(Ordering::SeqCst), 0, "no drops");

    drop(batch_tx);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = fs::remove_dir_all(&dir);
}

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

    drop(batch_tx);
    tokio::time::sleep(Duration::from_millis(500)).await;

    let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
    let result = engine.query("SELECT count(*) AS cnt FROM logs").await.expect("query");
    assert_eq!(
        result.rows[0][0],
        serde_json::json!(total_expected),
        "all {total_expected} events must appear exactly once",
    );

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_backpressure_overload() {
    let dir = test_dir("backpressure");
    let writer = Writer::new(&dir, 10).expect("new writer");
    let dropped = writer.dropped_events();
    writer.mark_startup_done();

    let (batch_tx, batch_rx) = mpsc::channel(256);
    let _handle = writer.spawn(batch_rx, Duration::from_secs(60));

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

#[tokio::test]
async fn test_capacity_check_ordered_within_group() {
    let dir = test_dir("cap_ordered_group");
    let writer = Writer::new(&dir, 3).expect("new writer");
    let dropped = writer.dropped_events();
    writer.mark_startup_done();

    let (batch_tx, batch_rx) = mpsc::channel(32);
    let _handle = writer.spawn(batch_rx, Duration::from_secs(60));

    let count = 3;
    let mut rxs = Vec::with_capacity(count);
    for i in 0..count {
        let (tx, rx) = oneshot::channel();
        let batch = make_batch("svc", i as i64, 2);
        use prost::Message;
        let raw = bytes::Bytes::from(batch.encode_to_vec());
        batch_tx.send((batch, raw, tx)).await.unwrap();
        rxs.push(rx);
    }

    let mut accepted = 0i64;
    let mut rejected = 0i64;
    for rx in rxs {
        let resp = rx.await.unwrap();
        if resp.accepted {
            accepted += resp.events_count;
        } else {
            rejected += resp.events_count;
            assert_eq!(resp.error, "buffer_full", "rejection reason");
        }
    }

    assert_eq!(accepted, 2, "only 2 events should fit (1 batch of 2)");
    assert_eq!(rejected, 4, "remaining 4 events should be rejected");
    assert_eq!(
        dropped.load(Ordering::SeqCst) as i64,
        rejected,
        "dropped_events counter matches rejected count"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_flush_lock_not_held_during_parquet_write() {
    let dir = test_dir("flush_lock_duration");
    let writer = Writer::new(&dir, 100_000).expect("new writer");

    let wal = writer.wal.clone();
    let buffer = writer.buffer.clone();
    let dedup_cache = writer.dedup_cache.clone();
    let dropped = writer.dropped_events();
    let startup_done = std::sync::atomic::AtomicBool::new(true);

    for i in 0..5 {
        let batch = make_batch("svc", i, 3);
        use prost::Message;
        let raw_bytes = bytes::Bytes::from(batch.encode_to_vec());
        handle_batch(&wal, &buffer, &dedup_cache, batch, raw_bytes, 100_000, &dropped, &startup_done, None);
    }

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
    };

    assert!(had_data, "should have extracted data");

    let slow_sleep = Duration::from_millis(100);

    let wal2 = wal.clone();

    let slow_flush = tokio::task::spawn_blocking(move || {
        std::thread::sleep(slow_sleep);

        let w = wal2.lock().expect("wal lock");
        w.delete_segments_up_to(sealed_seq).ok();
    });

    tokio::time::sleep(Duration::from_millis(20)).await;

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

#[tokio::test]
async fn test_crash_recovery_concurrent() {
    let dir = test_dir("crash_recovery_concurrent");

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

#[test]
fn test_startup_recovery_empty_wal() {
    let dir = test_dir("startup_empty_wal");
    let writer = Writer::new(&dir, 100_000).expect("new writer");
    writer.run_startup(1000).expect("run_startup");

    let replayed = wal::replay_segments(&dir).expect("replay");
    assert!(replayed.is_empty(), "no WAL data after startup");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_startup_recovery_basic_replay() {
    let dir = test_dir("startup_basic_replay");

    {
        let mut wal = wal::WalWriter::open(&dir).expect("open wal");
        let batch = make_unique_batch("svc", 1, &[1, 2, 3]);
        wal.append(&batch).expect("append");
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::mem::ManuallyDrop::new(wal);
    }

    {
        let writer = Writer::new(&dir, 100_000).expect("new writer");
        writer.run_startup(1000).expect("run_startup");
    }

    let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let result = rt.block_on(engine.query("SELECT count(*) AS cnt FROM logs")).expect("query");
    assert_eq!(result.rows[0][0], serde_json::json!(3i64));

    let replayed = wal::replay_segments(&dir).expect("replay");
    assert!(replayed.is_empty(), "WAL should be empty after startup recovery");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_startup_recovery_dedup_skips_duplicates() {
    let dir = test_dir("startup_dedup_skip");

    {
        let batch = make_unique_batch("svc", 1, &[1, 2, 3]);
        flush::flush_parquet_only(&dir, &[batch]).expect("flush");
    }

    {
        let mut wal = wal::WalWriter::open(&dir).expect("open wal");
        let batch = make_unique_batch("svc", 2, &[1, 2, 3, 4]);
        wal.append(&batch).expect("append");
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::mem::ManuallyDrop::new(wal);
    }

    {
        let writer = Writer::new(&dir, 100_000).expect("new writer");
        writer.run_startup(1000).expect("run_startup");
    }

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

#[test]
fn test_startup_recovery_partial_dedup() {
    let dir = test_dir("startup_partial_dedup");

    {
        let batch = make_unique_batch("svc", 1, &[1, 2, 3]);
        flush::flush_parquet_only(&dir, &[batch]).expect("flush");
    }

    {
        let mut wal = wal::WalWriter::open(&dir).expect("open wal");
        let batch = make_unique_batch("svc", 2, &[2, 3, 4, 5]);
        wal.append(&batch).expect("append");
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::mem::ManuallyDrop::new(wal);
    }

    {
        let writer = Writer::new(&dir, 100_000).expect("new writer");
        writer.run_startup(1000).expect("run_startup");
    }

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

#[test]
fn test_startup_recovery_cleanup_temp() {
    let dir = test_dir("startup_cleanup_temp");

    {
        let mut wal = wal::WalWriter::open(&dir).expect("open wal");
        let batch = make_unique_batch("svc", 1, &[1, 2]);
        wal.append(&batch).expect("append");
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::mem::ManuallyDrop::new(wal);
    }

    let tmp_dir = dir.join("data").join("table_type=logs").join("service=svc").join("date=2023-11-15");
    fs::create_dir_all(&tmp_dir).expect("create tmp dir");
    let tmp_file = tmp_dir.join("orphan.tmp");
    fs::write(&tmp_file, b"garbage").expect("write tmp");

    {
        let writer = Writer::new(&dir, 100_000).expect("new writer");
        writer.run_startup(1000).expect("run_startup");
    }

    assert!(!tmp_file.exists(), ".tmp file should be removed during startup");

    let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let result = rt.block_on(engine.query("SELECT count(*) AS cnt FROM logs")).expect("query");
    assert_eq!(result.rows[0][0], serde_json::json!(2i64));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_startup_recovery_no_data_loss() {
    let dir = test_dir("startup_no_data_loss");

    {
        let batch = make_unique_batch("svc", 1, &[1, 2, 3, 4, 5]);
        flush::flush_parquet_only(&dir, &[batch]).expect("flush");
    }

    {
        let mut wal = wal::WalWriter::open(&dir).expect("open wal");
        let batch = make_unique_batch("svc", 2, &[3, 4, 5, 6, 7]);
        wal.append(&batch).expect("append");
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::mem::ManuallyDrop::new(wal);
    }

    {
        let writer = Writer::new(&dir, 100_000).expect("new writer");
        writer.run_startup(1000).expect("run_startup");
    }

    let engine = crate::query_engine::QueryEngine::new(&dir).expect("new engine");
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let result = rt.block_on(engine.query("SELECT count(*) AS cnt FROM logs")).expect("query");
    assert_eq!(
        result.rows[0][0],
        serde_json::json!(7i64),
        "all 7 events present exactly once"
    );

    let replayed = wal::replay_segments(&dir).expect("replay");
    assert!(replayed.is_empty(), "WAL fully check-pointed");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_ingest_gated_during_recovery() {
    let dir = test_dir("ingest_gated");
    let writer = Writer::new(&dir, 100_000).expect("new writer");

    let wal = writer.wal.clone();
    let buffer = writer.buffer.clone();
    let dedup_cache = writer.dedup_cache.clone();
    let dropped = writer.dropped_events();

    let batch1 = make_batch("svc", 1, 1);
    use prost::Message;
    let raw_bytes1 = bytes::Bytes::from(batch1.encode_to_vec());
    let resp1 = handle_batch(&wal, &buffer, &dedup_cache, batch1, raw_bytes1, 100_000, &dropped, &writer.startup_done, None);
    assert!(!resp1.accepted, "ingest should be rejected during recovery");
    assert_eq!(resp1.error, "startup_recovery_in_progress");

    writer.mark_startup_done();

    let batch2 = make_batch("svc", 2, 2);
    let raw_bytes2 = bytes::Bytes::from(batch2.encode_to_vec());
    let resp2 = handle_batch(&wal, &buffer, &dedup_cache, batch2, raw_bytes2, 100_000, &dropped, &writer.startup_done, None);
    assert!(resp2.accepted, "ingest should be accepted after recovery");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_startup_recovery_corrupt_manifest() {
    let dir = test_dir("startup_corrupt_manifest");

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

#[test]
fn test_startup_recovery_tmp_inside_reconciled_partition() {
    use std::fs;

    let dir = test_dir("startup_tmp_reconcile");

    let partition_dir = dir.join("data").join("table_type=logs")
        .join("service=svc").join("date=2023-06-15");
    fs::create_dir_all(&partition_dir).expect("create partition");

    let source_path = partition_dir.join("000001.parquet");
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

    let tmp_path = partition_dir.join("orphan.tmp");
    fs::write(&tmp_path, b"garbage").expect("write tmp");

    let writer = Writer::new(&dir, 100_000).expect("new writer");
    writer.run_startup(1000).expect("run_startup should succeed");

    assert!(!tmp_path.exists(), "stray .tmp should be removed during startup");

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
