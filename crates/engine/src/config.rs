//! Runtime configuration for the write path.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub type LiveBuffer = Arc<RwLock<Vec<arrow::record_batch::RecordBatch>>>;

/// Directory holding the `logs/` Parquet tree and the `wal/` directory.
///
/// Resolution order: `GREPLOG_DATA_DIR` when set and non-empty, then the
/// platform per-user data location (`~/.local/share/greplog` on Linux,
/// `~/Library/Application Support/greplog` on macOS, `%APPDATA%\greplog`
/// on Windows). A global location means every command sees the same data no
/// matter which working directory it runs from.
fn default_storage_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("GREPLOG_DATA_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    platform_data_home().join("greplog")
}

/// Per-user application-data directory for the current platform; `.`
/// when neither the platform variable nor `HOME` is set.
#[cfg(target_os = "macos")]
fn platform_data_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Library/Application Support")
}

/// Per-user application-data directory for the current platform; `.`
/// when neither the platform variable nor `HOME` is set.
#[cfg(all(unix, not(target_os = "macos")))]
fn platform_data_home() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg);
        }
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/share"))
        .unwrap_or_default()
}

/// Per-user application-data directory for the current platform; `.`
/// when the platform variable is unset.
#[cfg(target_os = "windows")]
fn platform_data_home() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Network interface both servers bind (e.g. `127.0.0.1`, `0.0.0.0`).
    pub bind_host: String,
    pub data_dir: PathBuf,
    pub wal_path: PathBuf,
    pub flush_row_limit: usize,
    pub flush_interval_secs: u64,
    pub compaction_run_interval_secs: u64,
    pub max_files_before_compaction: usize,
    /// Size at which a compacted file is sealed and excluded from further
    /// merges; without it every sweep rewrites the previous sweep's output.
    pub compaction_target_bytes: u64,
    pub retention_days: Option<u32>,
    pub retention_run_interval_secs: u64,
    pub query_timeout_secs: u64,
    pub max_query_rows: usize,
    pub mpsc_buffer_size: usize,
    pub crossbeam_buffer_size: usize,
    pub ingest_port: u16,
    pub dashboard_port: u16,
}

impl Default for EngineConfig {
    /// Out-of-the-box values for a developer machine. The storage cadence
    /// assumes query cost tracks the number of open Parquet files, so flushing
    /// every 30s and sweeping every 5min keeps a partition at roughly a dozen
    /// files while bounding how long an acknowledged row sits only in the WAL.
    fn default() -> Self {
        let storage_root = default_storage_root();
        Self {
            bind_host: String::from("127.0.0.1"),
            data_dir: storage_root.join("logs"),
            wal_path: storage_root.join("wal/current.wal"),
            flush_row_limit: 10_000,
            flush_interval_secs: 30,
            compaction_run_interval_secs: 300,
            max_files_before_compaction: 8,
            compaction_target_bytes: 128 * 1024 * 1024,
            retention_days: Some(30),
            retention_run_interval_secs: 3600,
            query_timeout_secs: 30,
            max_query_rows: 10_000,
            mpsc_buffer_size: 1_024,
            crossbeam_buffer_size: 1_024,
            ingest_port: 5050,
            dashboard_port: 3000,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    use super::EngineConfig;

    #[test]
    fn defaults_live_under_one_storage_root() {
        let config = EngineConfig::default();
        assert_eq!(
            config.data_dir.file_name(),
            Some(OsStr::new("logs")),
            "Parquet tree lives under logs/ inside the storage root"
        );
        assert_eq!(
            config.wal_path.parent().and_then(Path::parent),
            config.data_dir.parent(),
            "wal/ sits beside logs/ under the same storage root"
        );
        assert_eq!(config.bind_host, "127.0.0.1");
        assert_eq!(config.retention_days, Some(30));
        assert_eq!(config.retention_run_interval_secs, 3600);
        assert!(
            config.flush_row_limit > 0,
            "flush threshold must allow progress"
        );
        assert!(
            config.flush_interval_secs > 0,
            "periodic flush cadence must allow progress"
        );
        assert!(
            config.compaction_run_interval_secs > 0,
            "compaction cadence must allow progress"
        );
        assert!(
            config.max_files_before_compaction > 0,
            "compaction trigger must allow merging"
        );
        assert!(
            config.compaction_target_bytes > 0,
            "a zero seal size would exclude every file from merging"
        );
        assert!(
            config.retention_run_interval_secs > 0,
            "retention cadence must allow progress"
        );
        assert!(
            config.query_timeout_secs > 0,
            "query timeout must allow progress"
        );
        assert!(
            config.max_query_rows > 0,
            "query row cap must allow progress"
        );
        assert!(
            config.mpsc_buffer_size > 0,
            "ingest channel capacity must allow progress"
        );
        assert!(
            config.crossbeam_buffer_size > 0,
            "handoff capacity must allow progress"
        );
    }

    #[test]
    fn compaction_sweeps_faster_than_files_accumulate() {
        let config = EngineConfig::default();
        let flushes_between_sweeps =
            config.compaction_run_interval_secs / config.flush_interval_secs;
        assert!(
            flushes_between_sweeps <= config.max_files_before_compaction as u64 * 2,
            "a sweep every {}s against a flush every {}s leaves {} chunks per service \
             un-merged, well past the {} file merge threshold",
            config.compaction_run_interval_secs,
            config.flush_interval_secs,
            flushes_between_sweeps,
            config.max_files_before_compaction,
        );
    }

    #[test]
    fn cloning_preserves_an_isolated_copy() {
        let original = EngineConfig {
            data_dir: PathBuf::from("/tmp/logs"),
            ..EngineConfig::default()
        };
        let clone = original.clone();
        assert_eq!(clone.data_dir, original.data_dir);
        assert_eq!(clone.wal_path, original.wal_path);
        assert_eq!(clone.flush_row_limit, original.flush_row_limit);
        assert_eq!(clone.flush_interval_secs, original.flush_interval_secs);
        assert_eq!(
            clone.compaction_run_interval_secs,
            original.compaction_run_interval_secs
        );
        assert_eq!(
            clone.max_files_before_compaction,
            original.max_files_before_compaction
        );
        assert_eq!(clone.retention_days, original.retention_days);
        assert_eq!(
            clone.retention_run_interval_secs,
            original.retention_run_interval_secs
        );
        assert_eq!(clone.query_timeout_secs, original.query_timeout_secs);
        assert_eq!(clone.max_query_rows, original.max_query_rows);
        assert_eq!(clone.mpsc_buffer_size, original.mpsc_buffer_size);
        assert_eq!(clone.crossbeam_buffer_size, original.crossbeam_buffer_size);
        assert_eq!(clone.ingest_port, original.ingest_port);
        assert_eq!(clone.dashboard_port, original.dashboard_port);
    }

    #[test]
    fn struct_update_overrides_only_the_chosen_field() {
        let config = EngineConfig {
            flush_row_limit: 512,
            ..EngineConfig::default()
        };
        assert_eq!(config.flush_row_limit, 512);
        assert_eq!(config.data_dir, EngineConfig::default().data_dir);
        assert_eq!(
            config.mpsc_buffer_size,
            EngineConfig::default().mpsc_buffer_size
        );
    }
}
