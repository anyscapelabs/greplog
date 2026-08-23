//! The retention/TTL purger: removes expired `day=` partitions from disk.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Duration as ChronoDuration, NaiveDate, Utc};

use crate::config::EngineConfig;
use crate::error::EngineError;

#[derive(Debug, Clone)]
pub struct Retention {
    config: EngineConfig,
}

impl Retention {
    #[must_use]
    pub fn new(config: EngineConfig) -> Self {
        Self { config }
    }

    pub fn find_expired_partitions(&self) -> Result<Vec<PathBuf>, EngineError> {
        let Some(days) = self.config.retention_days else {
            return Ok(Vec::new());
        };
        let cutoff = Utc::now().date_naive() - ChronoDuration::days(i64::from(days));
        let mut expired = Vec::new();
        walk_for_day_partitions(&self.config.data_dir, cutoff, &mut expired)?;
        Ok(expired)
    }

    /// Removes every expired `day=` partition, returning how many were purged.
    /// Removal is best-effort: a failure is logged and the sweep continues.
    pub fn purge_expired(&self) -> Result<usize, EngineError> {
        let expired = self.find_expired_partitions()?;
        let mut purged = 0usize;
        for path in &expired {
            match fs::remove_dir_all(path) {
                Ok(()) => {
                    purged += 1;
                    tracing::info!(path = ?path, "purged expired day partition");
                }
                Err(error) => {
                    tracing::error!(?error, path = ?path, "failed to purge expired partition");
                }
            }
        }
        Ok(purged)
    }

    pub async fn start_background_loop(config: EngineConfig) {
        if config.retention_days.is_none() {
            return;
        }
        let mut sweep = tokio::time::interval(Duration::from_secs(
            config.retention_run_interval_secs.max(1),
        ));
        sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let retention = Retention::new(config);
        loop {
            sweep.tick().await;
            let retention = retention.clone();
            let outcome = tokio::task::spawn_blocking(move || retention.purge_expired()).await;
            match outcome {
                Ok(Ok(purged)) => tracing::info!("purged {purged} expired day partitions"),
                Ok(Err(error)) => tracing::error!(?error, "retention scan failed"),
                Err(error) => tracing::error!(?error, "retention worker joined with an error"),
            }
        }
    }
}

fn walk_for_day_partitions(
    dir: &Path,
    cutoff: NaiveDate,
    out: &mut Vec<PathBuf>,
) -> Result<(), EngineError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let mut subdirs = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(date) = day_partition_date(&path) {
            if date < cutoff {
                out.push(path);
            }
        } else {
            subdirs.push(path);
        }
    }
    for subdir in subdirs {
        walk_for_day_partitions(&subdir, cutoff, out)?;
    }
    Ok(())
}

fn day_partition_date(dir: &Path) -> Option<NaiveDate> {
    let day = dir.file_name()?.to_str()?.strip_prefix("day=")?;
    let month = dir.parent()?.file_name()?.to_str()?.strip_prefix("month=")?;
    let year = dir
        .parent()?
        .parent()?
        .file_name()?
        .to_str()?
        .strip_prefix("year=")?;
    let year = year.parse().ok()?;
    let month: u32 = month.parse().ok()?;
    let day: u32 = day.parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::{Datelike, NaiveDate, Utc};
    use tempfile::tempdir;
    use super::{Retention, day_partition_date};
    use crate::config::EngineConfig;

    fn make_partition(root: &std::path::Path, date: NaiveDate) -> std::path::PathBuf {
        let path = root
            .join(format!("year={}", date.year()))
            .join(format!("month={:02}", date.month()))
            .join(format!("day={:02}", date.day()));
        fs::create_dir_all(&path).expect("create partition dir");
        path
    }

    #[test]
    fn day_partition_parser_reads_the_hive_hierarchy() {
        let dir = tempdir().expect("create temp dir");
        let path = make_partition(dir.path(), NaiveDate::from_ymd_opt(2026, 8, 9).unwrap());

        let parsed = day_partition_date(&path).expect("parse partition date");
        assert_eq!(parsed, NaiveDate::from_ymd_opt(2026, 8, 9).unwrap());
        assert!(
            day_partition_date(&dir.path().join("service=auth-api")).is_none(),
            "non-day directories must not parse"
        );
    }

    #[test]
    fn expired_partitions_are_flagged_and_young_ones_kept() {
        let dir = tempdir().expect("create temp dir");
        let root = dir.path().join("logs");
        let today = Utc::now().date_naive();
        let old = today - chrono::Duration::days(40);
        let fresh = today - chrono::Duration::days(1);
        let old_path = make_partition(&root, old);
        let _fresh_path = make_partition(&root, fresh);

        // A "year=9999" decoy that is not a real partition must be ignored.
        fs::create_dir_all(root.join("year=9999")).expect("create decoy");

        let retention = Retention::new(EngineConfig {
            data_dir: root.clone(),
            retention_days: Some(30),
            retention_run_interval_secs: 60,
            ..EngineConfig::default()
        });
        let expired = retention
            .find_expired_partitions()
            .expect("scan for expired partitions");
        assert_eq!(expired, vec![old_path], "only the 40-day-old partition expires");
    }

    #[test]
    fn disabled_retention_never_removes_data() {
        let dir = tempdir().expect("create temp dir");
        let root = dir.path().join("logs");
        let old = Utc::now().date_naive() - chrono::Duration::days(400);
        make_partition(&root, old);

        let retention = Retention::new(EngineConfig {
            data_dir: root.clone(),
            retention_days: None,
            retention_run_interval_secs: 60,
            ..EngineConfig::default()
        });
        let expired = retention.find_expired_partitions().expect("scan");
        assert!(expired.is_empty(), "None retention must purge nothing");
        assert_eq!(retention.purge_expired().expect("purge"), 0);
        assert!(root.exists(), "data_dir must survive a disabled retention sweep");
    }

    #[test]
    fn purge_removes_only_expired_directories() {
        let dir = tempdir().expect("create temp dir");
        let root = dir.path().join("logs");
        let today = Utc::now().date_naive();
        let old_path = make_partition(&root, today - chrono::Duration::days(45));
        let fresh_path = make_partition(&root, today - chrono::Duration::days(2));

        let retention = Retention::new(EngineConfig {
            data_dir: root.clone(),
            retention_days: Some(30),
            retention_run_interval_secs: 60,
            ..EngineConfig::default()
        });
        assert_eq!(retention.purge_expired().expect("purge"), 1);
        assert!(!old_path.exists(), "expired partition must be removed");
        assert!(fresh_path.exists(), "fresh partition must survive");
    }
}