use anyhow::{bail, Result};
use bytes::Bytes;
use crc32fast::Hasher;
use greplog_core::gen::IngestBatch;
use prost::Message;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::debug;

const WAL_DIR: &str = "wal";
const DEFAULT_FSYNC_INTERVAL: Duration = Duration::from_millis(50);
const DEFAULT_MAX_SEGMENT_SIZE: u64 = 16 * 1024 * 1024;

struct WalInner {
    current_file: Option<File>,
    current_path: PathBuf,
    current_seq: u64,
    current_size: u64,
    pending_writes: bool,
}

/// A write-ahead log that appends framed records to sequential segment files.
///
/// Record framing: `[4-byte LE total_len][4-byte CRC32(payload)][payload bytes]`
/// where `total_len` includes the CRC32 field (i.e. total_len = 4 + payload.len()).
///
/// fsync is performed by a background thread on a configurable interval
/// (default 50ms), not on every append. This means there is a deliberate
/// data-loss window of at most one interval if the process is killed
/// between syncs.
///
/// Segment files are named `<zero-padded-seq>.wal` (e.g. `000001.wal`) and
/// are rotated automatically when the active segment exceeds `max_segment_size`
/// (default 16 MB).
pub struct WalWriter {
    dir: PathBuf,
    inner: Arc<Mutex<WalInner>>,
    max_segment_size: u64,
    next_seq: u64,
    stop_flag: Arc<AtomicBool>,
    sync_thread: Option<JoinHandle<()>>,
}

impl WalWriter {
    pub fn open(dir: &Path) -> Result<Self> {
        Self::with_config(dir, DEFAULT_FSYNC_INTERVAL, DEFAULT_MAX_SEGMENT_SIZE)
    }

    pub fn with_config(
        dir: &Path,
        fsync_interval: Duration,
        max_segment_size: u64,
    ) -> Result<Self> {
        let wal_dir = dir.join(WAL_DIR);
        fs::create_dir_all(&wal_dir)?;

        let next_seq = highest_segment(&wal_dir)? + 1;
        let file_path = wal_dir.join(format!("{:06}.wal", next_seq));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(&file_path)?;

        let current_size = file.metadata().ok().map_or(0, |m| m.len());

        let inner = Arc::new(Mutex::new(WalInner {
            current_file: Some(file),
            current_path: file_path,
            current_seq: next_seq,
            current_size,
            pending_writes: false,
        }));

        debug!(
            "WAL opened at seq {} (fsync interval {:?}, max segment size {})",
            next_seq, fsync_interval, max_segment_size
        );

        let stop_flag = Arc::new(AtomicBool::new(false));
        let sync_stop = stop_flag.clone();
        let sync_inner = inner.clone();

        let sync_handle = thread::Builder::new()
            .name("greplog-wal-fsync".into())
            .spawn(move || loop {
                // Park with timeout so we can be interrupted immediately on shutdown.
                thread::park_timeout(fsync_interval);

                if sync_stop.load(Ordering::Relaxed) {
                    break;
                }

                let mut guard = match sync_inner.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };

                if guard.pending_writes {
                    if let Some(ref mut file) = guard.current_file {
                        let _ = file.sync_all();
                    }
                    guard.pending_writes = false;
                }
            })?;

        Ok(Self {
            dir: wal_dir,
            inner,
            max_segment_size,
            next_seq: next_seq + 1,
            stop_flag,
            sync_thread: Some(sync_handle),
        })
    }

    /// Append a framed record to the WAL.
    ///
    /// Encodes `batch` via prost, prepends length and CRC32, and writes to the
    /// active segment. Does not fsync — the background sync thread owns that.
    ///
    /// If the active segment has reached the size threshold, rotates to a new
    /// segment before appending.
    pub fn append(&mut self, batch: &IngestBatch) -> Result<()> {
        let payload = batch.encode_to_vec();
        let crc = crc32(&payload);

        let frame_len = 4 + 4 + payload.len();
        let mut buf = Vec::with_capacity(frame_len);
        buf.extend_from_slice(&(4u32 + payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&crc.to_le_bytes());
        buf.extend_from_slice(&payload);

        let mut inner = lock_inner(&self.inner)?;

        if inner.current_size + frame_len as u64 > self.max_segment_size {
            drop(inner);
            self.rotate_internal()?;
            inner = lock_inner(&self.inner)?;
        }

        let file = match inner.current_file.as_mut() {
            Some(f) => f,
            None => bail!("WAL is not open"),
        };
        file.write_all(&buf)?;
        inner.current_size += frame_len as u64;
        inner.pending_writes = true;

        Ok(())
    }

    /// Force-close the current segment and open a new one with the next sequence number.
    pub fn rotate(&mut self) -> Result<()> {
        self.rotate_internal()
    }

    fn rotate_internal(&mut self) -> Result<()> {
        let mut inner = lock_inner(&self.inner)?;

        if let Some(file) = inner.current_file.take() {
            let _ = file.sync_all();
            drop(file);
        }

        let file_path = self.dir.join(format!("{:06}.wal", self.next_seq));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(&file_path)?;

        inner.current_file = Some(file);
        inner.current_path = file_path;
        inner.current_seq = self.next_seq;
        inner.current_size = 0;

        self.next_seq += 1;

        debug!("WAL rotated to seq {}", self.next_seq - 1);
        Ok(())
    }

    /// Force an immediate fsync on the active segment file.
    pub fn flush_fsync(&mut self) -> Result<()> {
        let mut inner = lock_inner(&self.inner)?;
        if let Some(ref mut file) = inner.current_file {
            file.sync_all()?;
        }
        inner.pending_writes = false;
        Ok(())
    }

    /// The sequence number of the currently active segment.
    pub fn current_seq(&self) -> u64 {
        lock_inner(&self.inner)
            .ok()
            .map_or(0, |inner| inner.current_seq)
    }

    /// Flush (final sync), stop the background sync thread, and close the WAL.
    pub fn close(mut self) -> Result<()> {
        self.flush_fsync()?;
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(ref handle) = self.sync_thread {
            handle.thread().unpark();
        }
        if let Some(handle) = self.sync_thread.take() {
            let _ = handle.join();
        }
        Ok(())
    }
}

impl Drop for WalWriter {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(ref handle) = self.sync_thread {
            handle.thread().unpark();
        }
        if let Some(handle) = self.sync_thread.take() {
            let _ = handle.join();
        }

        if let Ok(mut inner) = self.inner.lock() {
            if let Some(ref mut file) = inner.current_file {
                let _ = file.sync_all();
            }
        }
    }
}

/// Replay all WAL segments, returning decoded batches in order with their
/// segment sequence number.
///
/// Segments are processed oldest-to-newest. Within each segment, records are
/// read sequentially. On encountering a corrupt record (CRC mismatch) or a
/// truncated record (length prefix exceeds remaining bytes, including
/// mid-header truncation), replay stops and returns everything successfully
/// parsed up to that point. Replay does not return an error for corruption
/// — it is an expected, normal case.
pub fn replay_segments(dir: &Path) -> Result<Vec<(IngestBatch, u64)>> {
    let wal_dir = dir.join(WAL_DIR);
    if !wal_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = fs::read_dir(&wal_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map_or(false, |n| n.ends_with(".wal"))
        })
        .collect();

    entries.sort_by_key(|e| {
        e.file_name()
            .to_str()
            .and_then(|n| n.trim_end_matches(".wal").parse::<u64>().ok())
            .unwrap_or(0)
    });

    let mut result = Vec::new();

    for entry in &entries {
        let data = match fs::read(entry.path()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let seq = entry
            .file_name()
            .to_str()
            .and_then(|n| n.trim_end_matches(".wal").parse::<u64>().ok())
            .unwrap_or(0);

        let mut offset = 0usize;
        while offset + 8 <= data.len() {
            let total_len = match data[offset..offset + 4].try_into() {
                Ok(arr) => u32::from_le_bytes(arr) as usize,
                Err(_) => break,
            };

            let end = match offset
                .checked_add(4)
                .and_then(|v| v.checked_add(total_len))
            {
                Some(v) => v,
                None => break,
            };

            if end > data.len() {
                break;
            }

            let stored_crc = match data[offset + 4..offset + 8].try_into() {
                Ok(arr) => u32::from_le_bytes(arr),
                Err(_) => break,
            };

            let payload = &data[offset + 8..end];
            if crc32(payload) != stored_crc {
                break;
            }

            if let Ok(batch) = IngestBatch::decode(Bytes::copy_from_slice(payload)) {
                result.push((batch, seq));
            }

            offset = end;
        }
    }

    Ok(result)
}

fn highest_segment(dir: &Path) -> Result<u64> {
    let mut max = 0u64;
    if !dir.exists() {
        return Ok(0);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(".wal") {
                if let Ok(n) = name.trim_end_matches(".wal").parse::<u64>() {
                    if n > max {
                        max = n;
                    }
                }
            }
        }
    }
    Ok(max)
}

fn lock_inner(inner: &Arc<Mutex<WalInner>>) -> Result<std::sync::MutexGuard<'_, WalInner>> {
    inner.lock().map_err(|e| anyhow::anyhow!("WAL mutex poisoned: {}", e))
}

fn crc32(data: &[u8]) -> u32 {
    let mut h = Hasher::new();
    h.update(data);
    h.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use greplog_core::gen::IngestBatch;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("greplog_wal_test_{}_{}", name, std::process::id()));
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

    /// Round-trip: append several records, replay, confirm back in order.
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

    /// Segment rotation: small threshold forces a new segment, both replay together.
    #[test]
    fn test_segment_rotation() {
        let dir = temp_dir("rotation");
        let mut wal =
            WalWriter::with_config(&dir, Duration::from_secs(60), 100).expect("open WAL");

        for i in 0..20 {
            wal.append(&make_batch(i)).expect("append");
        }
        wal.close().expect("close");

        let entries: Vec<_> = fs::read_dir(dir.join("wal"))
            .expect("read wal dir")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map_or(false, |n| n.ends_with(".wal"))
            })
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

    /// Truncation in payload: slice off mid-record, confirm valid records before it survive.
    #[test]
    fn test_truncation_recovery() {
        let dir = temp_dir("truncation_payload");
        let mut wal =
            WalWriter::with_config(&dir, Duration::from_secs(60), 999_999).expect("open WAL");

        wal.append(&make_batch(1)).expect("append 1");
        wal.append(&make_batch(2)).expect("append 2");

        let wal_dir = dir.join("wal");
        let seg_path = wal_dir.join("000001.wal");
        let original_len = fs::metadata(&seg_path).expect("metadata").len();

        wal.append(&make_batch(3)).expect("append 3");
        wal.close().expect("close");

        let len_after_3 = fs::metadata(&seg_path).expect("metadata").len();
        assert!(len_after_3 > original_len, "file should have grown");

        // Truncate the file back to where it was before record 3
        let file = OpenOptions::new().write(true).open(&seg_path).expect("open");
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

    /// Checksum mismatch: corrupt payload bytes, confirm replay stops before corrupt record.
    #[test]
    fn test_checksum_mismatch() {
        let dir = temp_dir("checksum");
        let mut wal =
            WalWriter::with_config(&dir, Duration::from_secs(60), 999_999).expect("open WAL");

        wal.append(&make_batch(1)).expect("append 1");
        wal.append(&make_batch(2)).expect("append 2");
        wal.append(&make_batch(3)).expect("append 3");
        wal.close().expect("close");

        let wal_dir = dir.join("wal");
        let seg_path = wal_dir.join("000001.wal");
        let mut data = fs::read(&seg_path).expect("read");

        // Skip past frame 1's bytes to land inside frame 2's payload
        let frame1_total = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        let frame2_offset = 4 + frame1_total;
        let corrupt_at = frame2_offset + 8 + 5; // 8=len+crc header, 5=into payload
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

    /// Header-boundary truncation: file ends partway through the header bytes
    /// (not the payload) of the last record. Replay must not panic and must
    /// return everything parsed before that record.
    #[test]
    fn test_header_truncation_recovery() {
        let dir = temp_dir("truncation_header");
        let mut wal =
            WalWriter::with_config(&dir, Duration::from_secs(60), 999_999).expect("open WAL");

        wal.append(&make_batch(1)).expect("append 1");
        wal.append(&make_batch(2)).expect("append 2");

        let wal_dir = dir.join("wal");
        let seg_path = wal_dir.join("000001.wal");
        let original_len = fs::metadata(&seg_path).expect("metadata").len();

        wal.append(&make_batch(3)).expect("append 3");
        wal.close().expect("close");

        let len_after_3 = fs::metadata(&seg_path).expect("metadata").len();
        assert!(len_after_3 > original_len, "file should have grown");

        // Truncate to original_len + 2 bytes (partway through the header of record 3)
        let truncate_to = original_len + 2;
        let file = OpenOptions::new().write(true).open(&seg_path).expect("open");
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

    /// Burst-then-idle: the background sync thread should flush a burst even
    /// when no new appends follow before a crash-style drop.
    #[test]
    fn test_background_fsync_before_crash() {
        let dir = temp_dir("background_fsync");
        let fsync_interval = Duration::from_millis(20);
        let mut wal =
            WalWriter::with_config(&dir, fsync_interval, 999_999).expect("open WAL");

        // Append a burst of records
        wal.append(&make_batch(100)).expect("append burst");
        wal.append(&make_batch(200)).expect("append burst");
        wal.append(&make_batch(300)).expect("append burst");

        // Wait long enough for the background sync to fire at least once
        // after the last append (2x interval for safety).
        thread::sleep(fsync_interval * 2);

        // "Crash" — stop the background thread and leak the writer without
        // calling close() or allowing Drop to run a final sync.
        wal.stop_flag.store(true, Ordering::Relaxed);
        if let Some(ref handle) = wal.sync_thread {
            handle.thread().unpark();
        }
        if let Some(handle) = wal.sync_thread.take() {
            let _ = handle.join();
        }
        // Leak the writer — no Drop, no final sync. The data should already
        // be on disk from the background sync thread's last tick.
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
}
