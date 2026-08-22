//! Integration tests for the runtime configuration layer.
//!
//! Verifies that [`EngineConfig`] exposes sane default fallbacks so a fresh
//! deployment works out of the box, and that struct-update overrides behave as
//! expected when callers customize individual fields.

use std::path::PathBuf;

use greplog_engine::config::EngineConfig;

#[test]
fn defaults_fall_back_to_the_conventional_layout() {
    let config = EngineConfig::default();

    assert_eq!(config.data_dir, PathBuf::from("data/logs"));
    assert_eq!(config.wal_path, PathBuf::from("data/wal/current.wal"));
    assert_eq!(config.flush_row_limit, 10_000);
    assert!(
        config.mpsc_buffer_size > 0,
        "mpsc_buffer_size must be a usable channel capacity, got {}",
        config.mpsc_buffer_size
    );
    assert!(
        config.crossbeam_buffer_size > 0,
        "crossbeam_buffer_size must be a usable channel capacity, got {}",
        config.crossbeam_buffer_size
    );
}

#[test]
fn config_is_debug_printable() {
    let config = EngineConfig::default();
    let rendered = format!("{config:?}");
    assert!(rendered.contains("EngineConfig"), "Debug output must name the struct");
    assert!(rendered.contains("flush_row_limit"), "Debug output must show the fields");
}

#[test]
fn cloned_config_stays_an_independent_copy() {
    let original = EngineConfig {
        data_dir: PathBuf::from("/srv/greplog/chunks"),
        wal_path: PathBuf::from("/srv/greplog/prewrite.wal"),
        flush_row_limit: 512,
        flush_interval_secs: 30,
        compaction_run_interval_secs: 60,
        max_files_before_compaction: 3,
        compaction_target_bytes: 64 * 1024 * 1024,
        retention_days: Some(14),
        retention_run_interval_secs: 120,
        query_timeout_secs: 15,
        max_query_rows: 5_000,
        mpsc_buffer_size: 64,
        crossbeam_buffer_size: 128,
        ingest_port: 5050,
        dashboard_port: 3000,
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
    assert_eq!(
        clone.compaction_target_bytes,
        original.compaction_target_bytes
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
fn partial_overrides_keep_all_other_defaults() {
    let config = EngineConfig {
        flush_row_limit: 2_500,
        ..EngineConfig::default()
    };

    assert_eq!(config.flush_row_limit, 2_500);
    assert_eq!(config.data_dir, EngineConfig::default().data_dir);
    assert_eq!(config.wal_path, EngineConfig::default().wal_path);
    assert_eq!(config.mpsc_buffer_size, EngineConfig::default().mpsc_buffer_size);
    assert_eq!(
        config.crossbeam_buffer_size,
        EngineConfig::default().crossbeam_buffer_size
    );
}