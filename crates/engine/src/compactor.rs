//! The background compactor: merges small Parquet chunks into one file.
//!
//! [`Compactor`] walks the Hive-partitioned tree under `config.data_dir`
//! looking for leaf partitions holding more than `max_files_before_compaction`
//! chunks, then stream-merges each crowded partition into a single highly
//! compressed `compacted_<uuid>.parquet` file. Rows are moved with a
//! [`ParquetRecordBatchReader`](parquet::arrow::arrow_reader::ParquetRecordBatchReader)
//! feeding an [`ArrowWriter`] one batch at a time, so memory stays flat no
//! matter how many gigabytes of small files accumulate. Original chunks are
//! deleted only after the merged file is written, flushed to disk, and closed,
//! so a crash mid-compaction never loses rows.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::Duration;

use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use uuid::Uuid;

use crate::config::EngineConfig;
use crate::error::EngineError;

/// How many rows each streaming reader yields per read.
///
/// Small batches keep the working set flat: only one batch is materialized at a
/// time while a partition is being merged.
const COMPACTION_BATCH_SIZE: usize = 1024;

/// Merges crowded leaf partitions into a single highly compressed Parquet file.
#[derive(Debug, Clone)]
pub struct Compactor {
    config: EngineConfig,
}

impl Compactor {
    /// Creates a compactor anchored at the configured `data_dir`.
    #[must_use]
    pub fn new(config: EngineConfig) -> Self {
        Self { config }
    }

    /// Returns every leaf partition holding more than `max_files_before_compaction`
    /// Parquet chunks.
    ///
    /// A missing `data_dir` (fresh engine, nothing flushed yet) is treated as an
    /// empty scan and not an error, so the background loop can start before any
    /// data exists.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::IoError`] if the tree cannot be walked.
    pub fn find_partitions_to_compact(&self) -> Result<Vec<PathBuf>, EngineError> {
        let mut partitions = Vec::new();
        walk_for_partitions(
            &self.config.data_dir,
            self.config.max_files_before_compaction,
            &mut partitions,
        )?;
        Ok(partitions)
    }

    /// Stream-merges the Parquet chunks of `partition_dir` into one file.
    ///
    /// All `.parquet` files are opened one at a time and streamed batch by batch
    /// into an [`ArrowWriter`] using Zstandard compression. The merged file is
    /// written to a temp name, then atomically renamed, and only then are the
    /// original chunks deleted — so the partition never exists in a partially
    /// merged state and concurrent readers never see a partial file. Already-
    /// compact partitions (at or under the merge threshold) are left untouched.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::IoError`] if a chunk cannot be opened or removed,
    /// or [`EngineError::ParquetError`] or [`EngineError::ArrowError`] if a
    /// chunk cannot be decoded or re-encoded.
    pub fn compact_partition(&self, partition_dir: &Path) -> Result<(), EngineError> {
        let files = parquet_files(partition_dir)?;
        if files.len() <= self.config.max_files_before_compaction {
            return Ok(());
        }

        let final_path = partition_dir.join(format!("compacted_{}.parquet", Uuid::new_v4()));
        let temp_path = partition_dir.join(format!(".tmp-compacted_{}.parquet", Uuid::new_v4()));
        let output = File::create(&temp_path)?;
        let properties = WriterProperties::builder()
            .set_compression(Compression::ZSTD(ZstdLevel::default()))
            .build();

        let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(&files[0])?)?
            .with_batch_size(COMPACTION_BATCH_SIZE);
        let schema = builder.schema().clone();
        let reader = builder.build()?;
        let mut writer = ArrowWriter::try_new(output, schema, Some(properties))?;
        for batch in reader {
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
        writer.close()?;
        // Atomic rename: the compacted file is only visible at its final name
        // after the writer is fully closed and synced.
        std::fs::rename(&temp_path, &final_path)?;

        File::open(&final_path)?.sync_all()?;
        for path in &files {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Runs compaction forever: every `compaction_run_interval_secs` the tree is
    /// scanned and each crowded partition is merged on a blocking thread.
    ///
    /// Failures are logged, never panicked on: a scan or merge error skips to
    /// the next sweep so compaction is retried on the following interval.
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

/// Recursively collects leaf directories holding more than `max_files` Parquet
/// chunks into `out`, tolerating a missing root.
fn walk_for_partitions(
    dir: &Path,
    max_files: usize,
    out: &mut Vec<PathBuf>,
) -> Result<(), EngineError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let mut parquet_count = 0usize;
    let mut subdirs = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            subdirs.push(path);
        } else if path.extension().is_some_and(|ext| ext == "parquet") {
            parquet_count += 1;
        }
    }
    if parquet_count > max_files {
        out.push(dir.to_path_buf());
    }
    for subdir in subdirs {
        walk_for_partitions(&subdir, max_files, out)?;
    }
    Ok(())
}

/// Returns the row-group files directly inside `dir`, deterministically sorted.
fn parquet_files(dir: &Path) -> Result<Vec<PathBuf>, EngineError> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.is_dir() && path.extension().is_some_and(|ext| ext == "parquet") {
            files.push(path);
        }
    }
    files.sort_unstable();
    Ok(files)
}