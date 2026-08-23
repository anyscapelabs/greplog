//! Runtime configuration for the write path.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub type LiveBuffer = Arc<RwLock<Vec<arrow::record_batch::RecordBatch>>>;

#[derive(Debug, Clone)]
pub struct EngineConfig {
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
        Self {
            data_dir: PathBuf::from("data/logs"),
            wal_path: PathBuf::from("data/wal/current.wal"),
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
    use std::path::PathBuf;

    use super::EngineConfig;

    #[test]
    fn defaults_point_at_the_conventional_layout() {
        let config = EngineConfig::default();
        assert_eq!(config.data_dir, PathBuf::from("data/logs"));
        assert_eq!(config.wal_path, PathBuf::from("data/wal/current.wal"));
        assert_eq!(config.retention_days, Some(30));
        assert_eq!(config.retention_run_interval_secs, 3600);
        assert!(config.flush_row_limit > 0, "flush threshold must allow progress");
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
        assert!(config.mpsc_buffer_size > 0, "ingest channel capacity must allow progress");
        assert!(config.crossbeam_buffer_size > 0, "handoff capacity must allow progress");
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
        assert_eq!(config.data_dir, PathBuf::from("data/logs"));
        assert_eq!(config.mpsc_buffer_size, EngineConfig::default().mpsc_buffer_size);
    }
}