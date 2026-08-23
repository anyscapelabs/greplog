//! The background compactor: merges small Parquet chunks into large ones.
//!
//! Files at or above `compaction_target_bytes` are never re-merged, so each
//! partition converges to a few large sealed files plus a short tail of fresh
//! chunks and every byte is written a bounded number of times.
//!
//! Chunks are merged in ascending `timestamp_us` order (from Parquet
//! statistics, not filenames) so row groups get narrow min/max ranges, which
//! is what time-bounded queries prune on.
//!
//! A merge is published without a visible-duplicate window and every crash
//! point is recoverable:
//!
//! 1. write to `compacted_<uuid>.parquet.part` and fsync (invisible to scans);
//! 2. rename every source chunk to `<name>.superseded` (rows briefly
//!    invisible rather than duplicated);
//! 3. rename the `.part` into place (merged rows become visible);
//! 4. delete the superseded files.
//!
//! [`Compactor::recover_partition`] rolls a partition back when it finds a
//! leftover `.part`, and finishes the cleanup when it does not.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::Duration;

use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::{EnabledStatistics, WriterProperties};
use parquet::file::statistics::Statistics;
use uuid::Uuid;

use crate::config::EngineConfig;
use crate::error::EngineError;
use crate::schema::TIMESTAMP_COLUMN;

const COMPACTION_BATCH_SIZE: usize = 1024;

const COMPACTED_ROW_GROUP_ROWS: usize = 128 * 1024;

const SUPERSEDED_SUFFIX: &str = "superseded";

const PART_SUFFIX: &str = "part";

#[derive(Debug, Clone)]
pub struct Compactor {
    config: EngineConfig,
}

impl Compactor {
    #[must_use]
    pub fn new(config: EngineConfig) -> Self {
        Self { config }
    }

    pub fn find_partitions_to_compact(&self) -> Result<Vec<PathBuf>, EngineError> {
        let mut partitions = Vec::new();
        self.walk_for_partitions(&self.config.data_dir, &mut partitions)?;
        Ok(partitions)
    }

    fn walk_for_partitions(
        &self,
        dir: &Path,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), EngineError> {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };

        let mut mergeable = 0usize;
        let mut subdirs = Vec::new();
        let mut needs_recovery = false;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                subdirs.push(path);
                continue;
            }
            if has_suffix(&path, PART_SUFFIX) || has_suffix(&path, SUPERSEDED_SUFFIX) {
                needs_recovery = true;
                continue;
            }
            if !is_parquet(&path) {
                continue;
            }
            let size = entry.metadata().map_or(0, |meta| meta.len());
            if size < self.config.compaction_target_bytes {
                mergeable += 1;
            }
        }

        if needs_recovery || mergeable > self.config.max_files_before_compaction {
            out.push(dir.to_path_buf());
        }
        for subdir in subdirs {
            self.walk_for_partitions(&subdir, out)?;
        }
        Ok(())
    }

    pub fn recover_partition(&self, partition_dir: &Path) -> Result<(), EngineError> {
        let mut parts = Vec::new();
        let mut superseded = Vec::new();
        for entry in fs::read_dir(partition_dir)? {
            let path = entry?.path();
            if has_suffix(&path, PART_SUFFIX) {
                parts.push(path);
            } else if has_suffix(&path, SUPERSEDED_SUFFIX) {
                superseded.push(path);
            }
        }
        if parts.is_empty() && superseded.is_empty() {
            return Ok(());
        }

        let rolling_back = !parts.is_empty();
        for path in parts {
            tracing::warn!(?path, "discarding an unpublished compaction output");
            fs::remove_file(&path)?;
        }
        for path in superseded {
            if rolling_back {
                let restored = path.with_extension("");
                tracing::warn!(?path, ?restored, "restoring a superseded chunk after an interrupted compaction");
                fs::rename(&path, &restored)?;
            } else {
                fs::remove_file(&path)?;
            }
        }
        Ok(())
    }

    pub fn compact_partition(&self, partition_dir: &Path) -> Result<(), EngineError> {
        self.recover_partition(partition_dir)?;

        let files = self.mergeable_files(partition_dir)?;
        if files.len() <= self.config.max_files_before_compaction {
            return Ok(());
        }

        let merge_id = Uuid::new_v4();
        let temp_path = partition_dir.join(format!("compacted_{merge_id}.parquet.{PART_SUFFIX}"));
        let final_path = partition_dir.join(format!("compacted_{merge_id}.parquet"));

        write_merged(&files, &temp_path)?;
        publish_merged(&files, &temp_path, &final_path)?;
        Ok(())
    }

    /// Mergeable chunks of `dir`, ordered by first timestamp. Files already at
    /// or above `compaction_target_bytes` are excluded — pulling them back in
    /// is what turns compaction quadratic.
    fn mergeable_files(&self, dir: &Path) -> Result<Vec<PathBuf>, EngineError> {
        let mut candidates = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() || !is_parquet(&path) {
                continue;
            }
            if entry.metadata()?.len() >= self.config.compaction_target_bytes {
                continue;
            }
            candidates.push(path);
        }
        sort_by_first_timestamp(&mut candidates);
        Ok(candidates)
    }

    pub async fn start_background_loop(config: EngineConfig) {
        let mut sweep = tokio::time::interval(Duration::from_secs(
            config.compaction_run_interval_secs.max(1),
        ));
        sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let compactor = Compactor::new(config);
        loop {
            sweep.tick().await;
            let partitions = match compactor.find_partitions_to_compact() {
                Ok(partitions) => partitions,
                Err(error) => {
                    tracing::error!(?error, "compaction scan failed");
                    continue;
                }
            };
            for partition in &partitions {
                let compactor = compactor.clone();
                let partition_path = partition.clone();
                let outcome = tokio::task::spawn_blocking(move || {
                    compactor.compact_partition(&partition_path)
                })
                .await;
                match outcome {
                    Ok(Ok(())) => tracing::info!("compacted partition {partition:?}"),
                    Ok(Err(error)) => tracing::error!(?error, "compaction failed for {partition:?}"),
                    Err(error) => tracing::error!(?error, "compaction worker joined with an error"),
                }
            }
        }
    }
}

fn write_merged(files: &[PathBuf], temp_path: &Path) -> Result<(), EngineError> {
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .set_max_row_group_size(COMPACTED_ROW_GROUP_ROWS)
        .set_statistics_enabled(EnabledStatistics::Page)
        .build();

    let first = ParquetRecordBatchReaderBuilder::try_new(File::open(&files[0])?)?
        .with_batch_size(COMPACTION_BATCH_SIZE);
    let schema = first.schema().clone();
    let output = File::create(temp_path)?;
    let mut writer = ArrowWriter::try_new(output, schema, Some(properties))?;

    for batch in first.build()? {
        writer.write(&batch?)?;
    }
    for path in &files[1..] {
        let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?
            .with_batch_size(COMPACTION_BATCH_SIZE)
            .build()?;
        for batch in reader {
            writer.write(&batch?)?;
        }
    }

    // `close` writes the footer; the fsync that follows is what makes it safe
    // to hide the sources.
    let file = writer.into_inner()?;
    file.sync_all()?;
    Ok(())
}

fn publish_merged(
    files: &[PathBuf],
    temp_path: &Path,
    final_path: &Path,
) -> Result<(), EngineError> {
    let superseded: Vec<PathBuf> = files.iter().map(|path| superseded_path(path)).collect();
    for (source, hidden) in files.iter().zip(&superseded) {
        fs::rename(source, hidden)?;
    }

    fs::rename(temp_path, final_path)?;
    sync_dir(final_path);

    for path in &superseded {
        if let Err(error) = fs::remove_file(path) {
            // Rows are already published; a leftover file is invisible to
            // queries and cleaned up by the next sweep.
            tracing::warn!(?error, ?path, "failed to remove a superseded chunk");
        }
    }
    Ok(())
}

fn sync_dir(path: &Path) {
    let Some(dir) = path.parent() else {
        return;
    };
    match File::open(dir) {
        Ok(handle) => {
            if let Err(error) = handle.sync_all() {
                tracing::warn!(?error, ?dir, "failed to fsync partition directory");
            }
        }
        Err(error) => tracing::warn!(?error, ?dir, "failed to open partition directory for fsync"),
    }
}

fn superseded_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".");
    name.push(SUPERSEDED_SUFFIX);
    PathBuf::from(name)
}

fn is_parquet(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "parquet")
}

fn has_suffix(path: &Path, suffix: &str) -> bool {
    path.extension().is_some_and(|ext| ext == suffix)
}

fn sort_by_first_timestamp(files: &mut [PathBuf]) {
    let mut keyed: Vec<(Option<i64>, PathBuf)> = files
        .iter()
        .map(|path| (min_timestamp(path), path.clone()))
        .collect();
    keyed.sort_by(|(left_ts, left_path), (right_ts, right_path)| match (left_ts, right_ts) {
        (Some(left), Some(right)) => left.cmp(right).then_with(|| left_path.cmp(right_path)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left_path.cmp(right_path),
    });
    for (slot, (_, path)) in files.iter_mut().zip(keyed) {
        *slot = path;
    }
}

fn min_timestamp(path: &Path) -> Option<i64> {
    let file = File::open(path).ok()?;
    let metadata = ParquetRecordBatchReaderBuilder::try_new(file).ok()?.metadata().clone();
    let column = metadata
        .file_metadata()
        .schema_descr()
        .columns()
        .iter()
        .position(|descriptor| descriptor.name() == TIMESTAMP_COLUMN)?;

    metadata
        .row_groups()
        .iter()
        .filter_map(|group| match group.column(column).statistics() {
            Some(Statistics::Int64(stats)) if stats.has_min_max_set() => Some(*stats.min()),
            _ => None,
        })
        .min()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{has_suffix, is_parquet, superseded_path, PART_SUFFIX, SUPERSEDED_SUFFIX};

    #[test]
    fn only_published_parquet_files_are_scannable() {
        assert!(is_parquet(Path::new("a/chunk_1.parquet")));
        assert!(!is_parquet(Path::new("a/chunk_1.parquet.part")));
        assert!(!is_parquet(Path::new("a/chunk_1.parquet.superseded")));
        assert!(!is_parquet(Path::new("a/notes.txt")));
    }

    #[test]
    fn in_flight_suffixes_are_recognised() {
        assert!(has_suffix(Path::new("a/x.parquet.part"), PART_SUFFIX));
        assert!(has_suffix(
            Path::new("a/x.parquet.superseded"),
            SUPERSEDED_SUFFIX
        ));
        assert!(!has_suffix(Path::new("a/x.parquet"), PART_SUFFIX));
    }

    #[test]
    fn superseded_names_round_trip_back_to_the_original() {
        let original = PathBuf::from("a/chunk_17.parquet");
        let hidden = superseded_path(&original);
        assert_eq!(hidden, PathBuf::from("a/chunk_17.parquet.superseded"));
        assert_eq!(hidden.with_extension(""), original);
    }
}
