use anyhow::Result;
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use tracing::debug;

/// Write a `RecordBatch` atomically to a parquet file inside `dir`.
///
/// Creates the directory if needed. The file goes through a temp-file
/// → fsync → rename → directory-fsync sequence so that a crash at any
/// point either leaves a `.parquet.tmp` orphan (recoverable via startup
/// cleanup) or a fully‑written `.parquet` file, never a partial file at
/// the final path.
///
/// Returns the complete path of the final parquet file.
pub fn write_parquet_atomic(dir: &Path, batch: &RecordBatch) -> Result<PathBuf> {
    let unique_id = uuid::Uuid::new_v4().to_string();
    write_parquet_atomic_with_id(dir, &unique_id, batch)
}

/// Like `write_parquet_atomic` but uses a caller-supplied unique
/// identifier so the final path is predictable before the file is
/// written.  Used by compaction, which pre‑registers the path in the
/// manifest before writing the data.
pub fn write_parquet_atomic_with_id(
    dir: &Path,
    unique_id: &str,
    batch: &RecordBatch,
) -> Result<PathBuf> {
    fs::create_dir_all(dir)?;

    let tmp_path = dir.join(format!("part-{}.parquet.tmp", unique_id));
    let final_path = dir.join(format!("part-{}.parquet", unique_id));

    {
        let file = File::create(&tmp_path)?;
        let schema = batch.schema();
        let props = WriterProperties::builder().build();
        let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
        writer.write(batch)?;
        writer.close()?;
    }

    {
        let file = OpenOptions::new().read(true).open(&tmp_path)?;
        file.sync_all()?;
    }

    fs::rename(&tmp_path, &final_path)?;

    if let Ok(dir_file) = OpenOptions::new().read(true).open(dir) {
        dir_file.sync_all()?;
    }

    debug!("Wrote {}", final_path.display());
    Ok(final_path)
}
