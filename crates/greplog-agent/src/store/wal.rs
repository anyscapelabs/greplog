use anyhow::{bail, Result};
use bytes::Bytes;
use crc32fast::Hasher;
use greplog_core::gen::IngestBatch;
use prost::Message;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::debug;

const WAL_DIR: &str = "wal";
const DEFAULT_FSYNC_INTERVAL: Duration = Duration::from_millis(50);

pub struct WalWriter {
    dir: PathBuf,
    current_file: Option<(File, PathBuf, u64)>,
    next_seq: u64,
    last_fsync: Instant,
    fsync_interval: Duration,
}

impl WalWriter {
    pub fn open(dir: &Path) -> Result<Self> {
        Self::with_fsync_interval(dir, DEFAULT_FSYNC_INTERVAL)
    }

    pub fn with_fsync_interval(dir: &Path, fsync_interval: Duration) -> Result<Self> {
        let wal_dir = dir.join(WAL_DIR);
        fs::create_dir_all(&wal_dir)?;

        let next_seq = highest_segment(&wal_dir)? + 1;
        let file_path = wal_dir.join(format!("{:06}.wal", next_seq));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(&file_path)?;

        debug!("WAL opened at seq {} (fsync interval {:?})", next_seq, fsync_interval);
        Ok(Self {
            dir: wal_dir,
            current_file: Some((file, file_path, next_seq)),
            next_seq: next_seq + 1,
            last_fsync: Instant::now(),
            fsync_interval,
        })
    }

    pub fn append(&mut self, batch: &IngestBatch) -> Result<()> {
        let payload = batch.encode_to_vec();
        let crc = crc32(&payload);

        let total_len = 4 + payload.len();
        let mut buf = Vec::with_capacity(4 + 4 + payload.len());
        buf.extend_from_slice(&(total_len as u32).to_le_bytes());
        buf.extend_from_slice(&crc.to_le_bytes());
        buf.extend_from_slice(&payload);

        let (ref mut file, _, _) = self.current_file.as_mut().unwrap();
        file.write_all(&buf)?;

        if self.last_fsync.elapsed() >= self.fsync_interval {
            file.sync_all()?;
            self.last_fsync = Instant::now();
        }

        Ok(())
    }

    pub fn rotate(&mut self) -> Result<()> {
        if let Some((file, _path, _seq)) = self.current_file.take() {
            let mut file = file;
            file.sync_all()?;
            drop(file);
        }

        let file_path = self.dir.join(format!("{:06}.wal", self.next_seq));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(&file_path)?;
        self.current_file = Some((file, file_path, self.next_seq));
        self.next_seq += 1;
        self.last_fsync = Instant::now();

        debug!("WAL rotated to seq {}", self.next_seq - 1);
        Ok(())
    }

    pub fn flush_fsync(&mut self) -> Result<()> {
        if let Some((ref mut file, _, _)) = self.current_file {
            file.sync_all()?;
            self.last_fsync = Instant::now();
        }
        Ok(())
    }

    /// Delete all WAL segments with seq < keep_smaller_than.
    /// The current segment is never deleted.
    pub fn delete_older_than(&mut self, seq: u64) -> Result<()> {
        let entries = std::fs::read_dir(&self.dir)?;
        for entry in entries {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".wal") {
                    if let Ok(n) = name.trim_end_matches(".wal").parse::<u64>() {
                        if n < seq {
                            let _ = fs::remove_file(entry.path());
                            debug!("Deleted WAL segment {}", n);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn current_seq(&self) -> u64 {
        self.current_file.as_ref().map(|(_, _, s)| *s).unwrap_or(0)
    }

    pub fn close(mut self) -> Result<()> {
        self.flush_fsync()?;
        Ok(())
    }
}

impl Drop for WalWriter {
    fn drop(&mut self) {
        if let Some((ref mut file, _, _)) = self.current_file {
            let _ = file.sync_all();
        }
    }
}

/// Read all un-checkpointed WAL segments and return decoded batches.
pub fn replay_wal(
    dir: &Path,
    up_to_excluding_seq: u64,
    checkpoint_seq: u64,
) -> Result<Vec<(IngestBatch, u64)>> {
    let wal_dir = dir.join(WAL_DIR);
    if !wal_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = std::fs::read_dir(&wal_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name().to_str().map_or(false, |n| {
                n.ends_with(".wal") && {
                    if let Ok(n) = n.trim_end_matches(".wal").parse::<u64>() {
                        n >= checkpoint_seq && n < up_to_excluding_seq
                    } else {
                        false
                    }
                }
            })
        })
        .collect();
    entries.sort_by_key(|e| {
        e.file_name()
            .to_str()
            .and_then(|n| n.trim_end_matches(".wal").parse::<u64>())
            .unwrap_or(0)
    });

    let mut result = Vec::new();
    for entry in &entries {
        let data = fs::read(entry.path())?;
        let mut offset = 0;
        while offset + 8 <= data.len() {
            let total_len = u32::from_le_bytes(
                data[offset..offset + 4].try_into().unwrap(),
            ) as usize;
            if offset + 4 + total_len > data.len() {
                break;
            }
            let stored_crc = u32::from_le_bytes(
                data[offset + 4..offset + 8].try_into().unwrap(),
            );
            let payload = &data[offset + 8..offset + 4 + total_len];
            let computed = crc32(payload);
            if computed != stored_crc {
                break;
            }
            if let Ok(batch) = IngestBatch::decode(Bytes::copy_from_slice(payload)) {
                let seq = entry.file_name()
                    .to_str()
                    .and_then(|n| n.trim_end_matches(".wal").parse::<u64>())
                    .unwrap_or(0);
                result.push((batch, seq));
            }
            offset += 4 + total_len;
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
                    if n > max { max = n; }
                }
            }
        }
    }
    Ok(max)
}

fn crc32(data: &[u8]) -> u32 {
    let mut h = Hasher::new();
    h.update(data);
    h.finalize()
}
