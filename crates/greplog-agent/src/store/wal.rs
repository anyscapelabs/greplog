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
    pending_writes: Arc<AtomicBool>,
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
            .open(&file_path)?;

        let current_size = file.metadata().ok().map_or(0, |m| m.len());

        let inner = Arc::new(Mutex::new(WalInner {
            current_file: Some(file),
            current_path: file_path,
            current_seq: next_seq,
            current_size,
        }));

        debug!(
            "WAL opened at seq {} (fsync interval {:?}, max segment size {})",
            next_seq, fsync_interval, max_segment_size
        );

        let pending_writes = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::new(AtomicBool::new(false));

        let sync_pending = pending_writes.clone();
        let sync_stop = stop_flag.clone();
        let sync_inner = inner.clone();

        let sync_handle = thread::Builder::new()
            .name("greplog-wal-fsync".into())
            .spawn(move || loop {
                thread::park_timeout(fsync_interval);

                if sync_stop.load(Ordering::SeqCst) {
                    break;
                }

                if sync_pending.load(Ordering::SeqCst) {
                    let mut guard = match sync_inner.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };

                    if let Some(ref mut file) = guard.current_file {
                        if let Err(e) = file.sync_all() {
                            tracing::error!("WAL fsync failed: {}", e);
                            // Keep pending_writes set so it retries on the next tick.
                            continue;
                        }
                    }

                    sync_pending.store(false, Ordering::SeqCst);
                }
            })?;

        Ok(Self {
            dir: wal_dir,
            inner,
            max_segment_size,
            pending_writes,
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
    ///
    /// **Blocking I/O:** performs blocking file writes. Callers running inside
    /// an async/tokio context MUST invoke this via `spawn_blocking` (or an
    /// equivalent dedicated blocking-safe path) — calling it directly from an
    /// async task will block that task's tokio worker thread on disk I/O.
    #[allow(dead_code)]
    pub fn prepare_append(batch: &IngestBatch) -> Vec<u8> {
        let payload = batch.encode_to_vec();
        let crc = crc32(&payload);

        let frame_len = 4 + 4 + payload.len();
        let mut buf = Vec::with_capacity(frame_len);
        buf.extend_from_slice(&(4u32 + payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&crc.to_le_bytes());
        buf.extend_from_slice(&payload);
        buf
    }

    pub fn append_raw(&mut self, buf: &[u8]) -> Result<()> {
        let frame_len = buf.len();
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
        file.write_all(buf)?;
        inner.current_size += frame_len as u64;
        self.pending_writes.store(true, Ordering::SeqCst);

        Ok(())
    }

    /// Append multiple pre-encoded frames in a single `write_vectored` call.
    ///
    /// All frames are written atomically (from the kernel's perspective) in
    /// order.  If the combined size exceeds the segment threshold the segment
    /// is rotated before writing.
    pub fn append_raw_batch(&mut self, bufs: &[&[u8]]) -> Result<()> {
        if bufs.is_empty() {
            return Ok(());
        }
        let total_len: usize = bufs.iter().map(|b| b.len()).sum();
        let mut inner = lock_inner(&self.inner)?;

        if inner.current_size + total_len as u64 > self.max_segment_size {
            drop(inner);
            self.rotate_internal()?;
            inner = lock_inner(&self.inner)?;
        }

        let file = match inner.current_file.as_mut() {
            Some(f) => f,
            None => bail!("WAL is not open"),
        };
        let mut flat = Vec::with_capacity(total_len);
        for buf in bufs {
            flat.extend_from_slice(buf);
        }
        file.write_all(&flat)?;
        inner.current_size += total_len as u64;
        self.pending_writes.store(true, Ordering::SeqCst);

        Ok(())
    }

    #[allow(dead_code)]
    pub fn append(&mut self, batch: &IngestBatch) -> Result<()> {
        let buf = Self::prepare_append(batch);
        self.append_raw(&buf)
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
        self.pending_writes.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// The sequence number of the currently active segment.
    pub fn current_seq(&self) -> u64 {
        lock_inner(&self.inner)
            .ok()
            .map_or(0, |inner| inner.current_seq)
    }

    /// Flush (final sync), stop the background sync thread, and close the WAL.
    /// Delete all WAL segments with sequence number strictly less than `up_to`.
    /// The current active segment is never deleted.
    pub fn delete_segments_up_to(&self, up_to: u64) -> Result<()> {
        let current = self.current_seq();
        let entries = match fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let name = match entry.file_name().to_str() {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !name.ends_with(".wal") {
                continue;
            }
            if let Ok(seq) = name.trim_end_matches(".wal").parse::<u64>() {
                if seq < up_to && seq != current {
                    let _ = fs::remove_file(entry.path());
                    debug!("Deleted WAL segment {}", seq);
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn close(mut self) -> Result<()> {
        self.flush_fsync()?;
        self.shutdown_background();
        Ok(())
    }

    /// Stop the background sync thread and join it, without consuming `self`.
    /// Useful when the `WalWriter` is held behind an `Arc<Mutex<...>>`.
    pub fn shutdown_background(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(ref handle) = self.sync_thread {
            handle.thread().unpark();
        }
        if let Some(handle) = self.sync_thread.take() {
            let _ = handle.join();
        }
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
                .is_some_and(|n| n.ends_with(".wal"))
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
#[path = "../../tests/modules/store/wal.rs"]
mod tests;
