use super::*;
use greplog_core::gen::IngestBatch;

fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("greplog_wal_test_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn make_batch(seq: u64) -> IngestBatch {
    IngestBatch {
        service_name: "test-svc".into(),
        instance_id: "inst-1".into(),
        batch_seq: seq as i64,
        logs: Vec::new(),
        spans: Vec::new(),
        metrics: Vec::new(),
    }
}

#[test]
fn test_round_trip() {
    let dir = temp_dir("round_trip");
    let mut wal = WalWriter::open(&dir).expect("open WAL");

    wal.append(&make_batch(10)).expect("append 10");
    wal.append(&make_batch(20)).expect("append 20");
    wal.append(&make_batch(30)).expect("append 30");
    wal.close().expect("close WAL");

    let replayed = replay_segments(&dir).expect("replay");
    assert_eq!(replayed.len(), 3);
    assert_eq!(replayed[0].0.batch_seq, 10i64);
    assert_eq!(replayed[1].0.batch_seq, 20i64);
    assert_eq!(replayed[2].0.batch_seq, 30i64);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_segment_rotation() {
    let dir = temp_dir("rotation");
    let mut wal = WalWriter::with_config(&dir, Duration::from_secs(60), 100).expect("open WAL");

    for i in 0..20 {
        wal.append(&make_batch(i)).expect("append");
    }
    wal.close().expect("close");

    let entries: Vec<_> = fs::read_dir(dir.join("wal"))
        .expect("read wal dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".wal")))
        .collect();
    assert!(
        entries.len() >= 2,
        "expected at least 2 segments, got {}",
        entries.len()
    );

    let replayed = replay_segments(&dir).expect("replay");
    assert_eq!(replayed.len(), 20);
    for (i, (batch, _seq)) in replayed.iter().enumerate() {
        assert_eq!(batch.batch_seq, i as i64);
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_truncation_recovery() {
    let dir = temp_dir("truncation_payload");
    let mut wal = WalWriter::with_config(&dir, Duration::from_secs(60), 999_999).expect("open WAL");

    wal.append(&make_batch(1)).expect("append 1");
    wal.append(&make_batch(2)).expect("append 2");

    let wal_dir = dir.join("wal");
    let seg_path = wal_dir.join("000001.wal");
    let original_len = fs::metadata(&seg_path).expect("metadata").len();

    wal.append(&make_batch(3)).expect("append 3");
    wal.close().expect("close");

    let len_after_3 = fs::metadata(&seg_path).expect("metadata").len();
    assert!(len_after_3 > original_len, "file should have grown");

    let file = OpenOptions::new()
        .write(true)
        .open(&seg_path)
        .expect("open");
    file.set_len(original_len).expect("truncate");
    drop(file);

    let replayed = replay_segments(&dir).expect("replay");
    assert_eq!(
        replayed.len(),
        2,
        "only first two records should survive truncation"
    );
    assert_eq!(replayed[0].0.batch_seq, 1i64);
    assert_eq!(replayed[1].0.batch_seq, 2i64);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_checksum_mismatch() {
    let dir = temp_dir("checksum");
    let mut wal = WalWriter::with_config(&dir, Duration::from_secs(60), 999_999).expect("open WAL");

    wal.append(&make_batch(1)).expect("append 1");
    wal.append(&make_batch(2)).expect("append 2");
    wal.append(&make_batch(3)).expect("append 3");
    wal.close().expect("close");

    let wal_dir = dir.join("wal");
    let seg_path = wal_dir.join("000001.wal");
    let mut data = fs::read(&seg_path).expect("read");

    let frame1_total = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let frame2_offset = 4 + frame1_total;
    let corrupt_at = frame2_offset + 8 + 5;
    if corrupt_at < data.len() {
        data[corrupt_at] ^= 0xFF;
        fs::write(&seg_path, &data).expect("write corrupted data");
    }

    let replayed = replay_segments(&dir).expect("replay");
    assert_eq!(
        replayed.len(),
        1,
        "only first record should survive checksum corruption"
    );
    assert_eq!(replayed[0].0.batch_seq, 1i64);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_header_truncation_recovery() {
    let dir = temp_dir("truncation_header");
    let mut wal = WalWriter::with_config(&dir, Duration::from_secs(60), 999_999).expect("open WAL");

    wal.append(&make_batch(1)).expect("append 1");
    wal.append(&make_batch(2)).expect("append 2");

    let wal_dir = dir.join("wal");
    let seg_path = wal_dir.join("000001.wal");
    let original_len = fs::metadata(&seg_path).expect("metadata").len();

    wal.append(&make_batch(3)).expect("append 3");
    wal.close().expect("close");

    let len_after_3 = fs::metadata(&seg_path).expect("metadata").len();
    assert!(len_after_3 > original_len, "file should have grown");

    let truncate_to = original_len + 2;
    let file = OpenOptions::new()
        .write(true)
        .open(&seg_path)
        .expect("open");
    file.set_len(truncate_to).expect("truncate");
    drop(file);

    let replayed = replay_segments(&dir).expect("replay");
    assert_eq!(
        replayed.len(),
        2,
        "both valid records should survive header truncation"
    );
    assert_eq!(replayed[0].0.batch_seq, 1i64);
    assert_eq!(replayed[1].0.batch_seq, 2i64);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_close_then_drop() {
    let dir = temp_dir("close_then_drop");
    let mut wal = WalWriter::open(&dir).expect("open WAL");
    wal.append(&make_batch(1)).expect("append");
    wal.close().expect("close");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_background_fsync_before_crash() {
    let dir = temp_dir("background_fsync");
    let fsync_interval = Duration::from_millis(20);
    let mut wal = WalWriter::with_config(&dir, fsync_interval, 999_999).expect("open WAL");

    wal.append(&make_batch(100)).expect("append burst");
    wal.append(&make_batch(200)).expect("append burst");
    wal.append(&make_batch(300)).expect("append burst");

    thread::sleep(fsync_interval * 2);

    wal.stop_flag.store(true, Ordering::Relaxed);
    if let Some(ref handle) = wal.sync_thread {
        handle.thread().unpark();
    }
    if let Some(handle) = wal.sync_thread.take() {
        let _ = handle.join();
    }
    let _ = std::mem::ManuallyDrop::new(wal);

    let replayed = replay_segments(&dir).expect("replay");
    assert_eq!(
        replayed.len(),
        3,
        "burst should survive crash because background fsync already synced it"
    );
    assert_eq!(replayed[0].0.batch_seq, 100i64);
    assert_eq!(replayed[1].0.batch_seq, 200i64);
    assert_eq!(replayed[2].0.batch_seq, 300i64);

    let _ = fs::remove_dir_all(&dir);
}
