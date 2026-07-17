use anyhow::Result;
use arrow::compute::concat_batches;
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Name of the per‑partition manifest file stored inside each
/// `table_type=…/service=…/date=…` directory.
const MANIFEST_FILE: &str = ".compaction_manifest";

/// Target merged-file size in bytes (~128 MB).
const DEFAULT_TARGET_SIZE: u64 = 128 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct ManifestEntry {
    merged_file: String,
    source_files: Vec<String>,
    status: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Manifest {
    entries: Vec<ManifestEntry>,
}

/// Information about one completed compaction cycle.
pub struct CompactionManifest {
    pub merged_file: PathBuf,
    pub source_files: Vec<PathBuf>,
}

/// Result returned by `compact_partition`.
pub struct CompactionResult {
    pub files_compacted: usize,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compact a single partition directory.
///
/// Reads all small parquet files, merges them into one, writes the merged
/// file atomically, writes a manifest entry, deletes the source files,
/// and marks the entry as completed.  Uses the temp‑file → fsync → rename
/// → dir‑fsync pattern for both the merged data file and the manifest.
///
/// If `skip_today` is true and the partition directory corresponds to the
/// current UTC date, the function returns `Ok(0)` without doing anything.
pub fn compact_partition(
    dir: &Path,
    _target_size: u64,
    skip_today: bool,
) -> Result<CompactionResult> {
    if skip_today && is_today_partition(dir) {
        debug!("Skipping today's partition: {:?}", dir);
        return Ok(CompactionResult { files_compacted: 0 });
    }

    let sources = get_source_files(dir)?;
    if sources.len() < 2 {
        return Ok(CompactionResult { files_compacted: 0 });
    }

    let manifest = compact_phase1(dir, &sources)?;
    compact_phase2(dir, &manifest)?;

    Ok(CompactionResult {
        files_compacted: manifest.source_files.len(),
    })
}

/// Phase 1: merge source files, pre‑register the merge in the manifest
/// (*before* writing the merged file), write the merged file atomically,
/// then update the manifest status to `pending_delete`.
///
/// Ordering: manifest (pending_merge) → merged file → manifest
/// (pending_delete).  This guarantees the merged file is always
/// accounted for in the manifest as soon as it exists on disk, closing
/// the window that would otherwise create an orphan.
///
/// Does **not** delete any source files yet.  Exposed for testing
/// ordering guarantees.
pub fn compact_phase1(dir: &Path, sources: &[PathBuf]) -> Result<CompactionManifest> {
    let batches = read_parquet_files(sources)?;
    let merged = concatenate_batches(&batches)?;

    // Pre‑generate a unique ID so the merged file's path is known before
    // the write starts — the manifest entry is written first.
    let unique_id = uuid::Uuid::new_v4().to_string();
    let merged_name = format!("part-{}.parquet", unique_id);

    let source_names: Vec<String> = sources
        .iter()
        .map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();

    // 1. Write manifest entry FIRST with status "pending_merge".
    add_manifest_entry(
        dir,
        ManifestEntry {
            merged_file: merged_name.clone(),
            source_files: source_names.clone(),
            status: "pending_merge".into(),
        },
    )?;

    // 2. Write the merged file to the pre‑registered path.
    let merged_path =
        crate::store::io::write_parquet_atomic_with_id(dir, &unique_id, &merged)?;

    // 3. Update manifest: pending_merge → pending_delete.
    update_manifest_entry_status(dir, &merged_path, "pending_delete")?;

    Ok(CompactionManifest {
        merged_file: merged_path,
        source_files: sources.to_vec(),
    })
}

/// Phase 2: delete source files referenced by the manifest, then mark
/// the manifest entry as completed.
pub fn compact_phase2(dir: &Path, manifest: &CompactionManifest) -> Result<()> {
    for src in &manifest.source_files {
        if src.exists() {
            fs::remove_file(src)?;
        }
    }

    update_manifest_entry_status(
        dir,
        &manifest.merged_file,
        "completed",
    )?;

    Ok(())
}

/// Walk all partition directories under `base_dir/data/` and reconcile
/// any incomplete compaction manifests.
///
/// Handles two incomplete statuses:
///
/// `pending_merge` — the manifest entry was written but the merged file
/// may or may not exist yet:
/// - Merged file exists → delete sources, mark as `completed`.
/// - Merged file missing → pre‑registration that never materialised;
///   remove the entry.
///
/// `pending_delete` — the merged file is durable but some source files
/// may still need removal:
/// - Merged file exists → delete remaining sources, mark as `completed`.
/// - Merged file missing (shouldn't happen) → log warning, remove entry.
///
/// This function is **idempotent** – calling it twice is safe.
pub fn reconcile_compactions(base_dir: &Path) -> Result<()> {
    let data_dir = base_dir.join("data");
    if !data_dir.exists() {
        return Ok(());
    }

    let partition_dirs = find_partition_dirs(&data_dir);

    for part_dir in &partition_dirs {
        let manifest_path = part_dir.join(MANIFEST_FILE);
        if !manifest_path.exists() {
            continue;
        }

        let manifest_data = match fs::read_to_string(&manifest_path) {
            Ok(d) => d,
            Err(e) => {
                warn!("Cannot read manifest {:?}: {}", manifest_path, e);
                continue;
            }
        };

        let manifest: Manifest = match serde_json::from_str(&manifest_data) {
            Ok(m) => m,
            Err(e) => {
                warn!("Cannot parse manifest {:?}: {}", manifest_path, e);
                continue;
            }
        };

        let mut changed = false;
        let mut entries = manifest.entries;

        for entry in entries.iter_mut() {
            let merged_path = part_dir.join(&entry.merged_file);
            let merged_exists = merged_path.exists();

            match entry.status.as_str() {
                "pending_merge" => {
                    if merged_exists {
                        // Merged file was written but the status update
                        // to pending_delete crashed.  Finish the cycle.
                        for sname in &entry.source_files {
                            let src_path = part_dir.join(sname);
                            if src_path.exists() {
                                fs::remove_file(&src_path)?;
                            }
                        }
                        entry.status = "completed".into();
                        changed = true;
                        info!(
                            "Reconciled pending_merge (merged exists) in {:?}",
                            part_dir
                        );
                    } else {
                        // Pre‑registration that never materialised.
                        entry.status = "removed".into();
                        changed = true;
                        info!(
                            "Removed pending_merge (merged missing) in {:?}",
                            part_dir
                        );
                    }
                }
                "pending_delete" => {
                    if !merged_exists {
                        warn!(
                            "Compaction merged file missing, removing dangling entry: {} in {:?}",
                            entry.merged_file, part_dir
                        );
                        entry.status = "removed".into();
                        changed = true;
                        continue;
                    }

                    for sname in &entry.source_files {
                        let src_path = part_dir.join(sname);
                        if src_path.exists() {
                            fs::remove_file(&src_path)?;
                        }
                    }

                    entry.status = "completed".into();
                    changed = true;
                    info!("Reconciled compaction in {:?}", part_dir);
                }
                _ => {}
            }
        }

        if changed {
            entries.retain(|e| e.status != "removed");
            let new_manifest = Manifest { entries };
            write_manifest_atomic(part_dir, &new_manifest)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// List `.parquet` files in `dir` that are not currently referenced as
/// sources or merged files by any in‑flight manifest entry
/// (`pending_merge` or `pending_delete`).
fn get_source_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let manifest_path = dir.join(MANIFEST_FILE);
    let active_merged: HashSet<String>;
    let active_sources: HashSet<String>;

    if manifest_path.exists() {
        let data = fs::read_to_string(&manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&data)?;

        let merge_merged: HashSet<String> = manifest
            .entries
            .iter()
            .filter(|e| e.status == "pending_merge")
            .map(|e| e.merged_file.clone())
            .collect();

        let merge_sources: HashSet<String> = manifest
            .entries
            .iter()
            .filter(|e| e.status == "pending_merge")
            .flat_map(|e| e.source_files.clone())
            .collect();

        let delete_merged: HashSet<String> = manifest
            .entries
            .iter()
            .filter(|e| e.status == "pending_delete")
            .map(|e| e.merged_file.clone())
            .collect();

        let delete_sources: HashSet<String> = manifest
            .entries
            .iter()
            .filter(|e| e.status == "pending_delete")
            .flat_map(|e| e.source_files.clone())
            .collect();

        active_merged = merge_merged.union(&delete_merged).cloned().collect();
        active_sources = merge_sources.union(&delete_sources).cloned().collect();
    } else {
        active_merged = HashSet::new();
        active_sources = HashSet::new();
    }

    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            name.ends_with(".parquet")
                && !active_sources.contains(name)
                && !active_merged.contains(name)
                && name != MANIFEST_FILE
                && !name.ends_with(".tmp")
        })
        .collect();

    files.sort();
    Ok(files)
}

/// Read all rows from the given parquet files into a flat vector of
/// RecordBatches.
fn read_parquet_files(paths: &[PathBuf]) -> Result<Vec<RecordBatch>> {
    let mut all = Vec::new();
    for path in paths {
        let file = File::open(path)?;
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)?
            .build()?;
        for batch_result in reader {
            all.push(batch_result?);
        }
    }
    Ok(all)
}

/// Concatenate multiple RecordBatches into one.
fn concatenate_batches(batches: &[RecordBatch]) -> Result<RecordBatch> {
    if batches.is_empty() {
        anyhow::bail!("no record batches to concatenate");
    }
    let schema = batches[0].schema();
    let refs: Vec<&RecordBatch> = batches.iter().collect();
    concat_batches(&schema, refs.iter().copied())
        .map_err(|e| anyhow::anyhow!("concat batches: {}", e))
}

/// Read the existing manifest for a partition directory, or return an
/// empty manifest.
fn read_manifest(dir: &Path) -> Result<Manifest> {
    let path = dir.join(MANIFEST_FILE);
    if !path.exists() {
        return Ok(Manifest::default());
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

/// Atomically write a manifest to a partition directory (temp → fsync → rename).
fn write_manifest_atomic(dir: &Path, manifest: &Manifest) -> Result<()> {
    let tmp_path = dir.join(format!("{}.tmp", MANIFEST_FILE));
    let final_path = dir.join(MANIFEST_FILE);

    {
        let json = serde_json::to_string_pretty(manifest)?;
        let mut file = File::create(&tmp_path)?;
        use std::io::Write;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }

    fs::rename(&tmp_path, &final_path)?;

    if let Ok(dir_file) = OpenOptions::new().read(true).open(dir) {
        dir_file.sync_all()?;
    }

    Ok(())
}

/// Add a new entry to the partition manifest.
fn add_manifest_entry(dir: &Path, entry: ManifestEntry) -> Result<()> {
    let mut manifest = read_manifest(dir)?;
    manifest.entries.push(entry);
    write_manifest_atomic(dir, &manifest)
}

/// Update the status of a manifest entry identified by its merged file name.
fn update_manifest_entry_status(
    dir: &Path,
    merged_file: &Path,
    new_status: &str,
) -> Result<()> {
    let mut manifest = read_manifest(dir)?;
    let mname = merged_file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    for entry in &mut manifest.entries {
        if entry.merged_file == mname {
            entry.status = new_status.to_string();
            break;
        }
    }

    write_manifest_atomic(dir, &manifest)
}

/// Recursively find all leaf directories under `data_dir` that match
/// the `date=YYYY-MM-DD` pattern — these are partition directories.
fn find_partition_dirs(data_dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Check if this is a date=YYYY-MM-DD leaf
                if is_date_dir(&path) {
                    result.push(path);
                } else {
                    result.extend(find_partition_dirs(&path));
                }
            }
        }
    }
    result
}

fn is_date_dir(dir: &Path) -> bool {
    dir.file_name()
        .and_then(|n| n.to_str())
        .map_or(false, |n| {
            n.starts_with("date=") && n.len() == 15 // "date=YYYY-MM-DD"
        })
}

/// Return today's UTC date string in `YYYY-MM-DD` format.
fn today_date_string() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as i64;
    let days = if secs >= 0 {
        secs / 86400
    } else {
        (secs + 1) / 86400 - 1
    };
    civil_days_to_ymd(days)
}

/// Check if a partition directory is for today's date.
fn is_today_partition(dir: &Path) -> bool {
    let dirname = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let date_str = dirname.strip_prefix("date=").unwrap_or("");
    if date_str.is_empty() || date_str.len() != 10 {
        return false;
    }
    date_str == today_date_string()
}

/// Convert days since Unix epoch to `YYYY-MM-DD` using Howard Hinnant's
/// civil‑date algorithm.
fn civil_days_to_ymd(days: i64) -> String {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let day = doy - (153 * mp + 2) / 5 + 1;
    let year = if month <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", year, month, day)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::io::write_parquet_atomic;
    use arrow::array::*;
    use arrow::record_batch::RecordBatch;
    use greplog_core::arrow_schema;
    use std::sync::Arc;

    /// Create a one‑row log RecordBatch.
    fn make_log_batch(service: &str, message: &str) -> RecordBatch {
        let schema = arrow_schema::log_schema();
        let ts = 1_700_000_000_000_000;
        RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(StringArray::from(vec![uuid::Uuid::new_v4().to_string()])),
                Arc::new(StringArray::from(vec![service])),
                Arc::new(TimestampMicrosecondArray::from(vec![ts])),
                Arc::new(StringArray::from(vec![Some("info")])),
                Arc::new(StringArray::from(vec![None::<&str>])),
                Arc::new(StringArray::from(vec![Some(message)])),
                Arc::new(StringArray::from(vec![None::<&str>])),
            ],
        )
        .expect("build log batch")
    }

    /// Create a temp directory for a single test.
    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("greplog_compaction_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn make_small_batch(service: &str, file_idx: usize, rows: usize) -> RecordBatch {
        let schema = arrow_schema::log_schema();
        let ts = 1_700_000_000_000_000;
        let ids: Vec<String> = (0..rows).map(|_| uuid::Uuid::new_v4().to_string()).collect();
        let services: Vec<&str> = (0..rows).map(|_| service).collect();
        let timestamps: Vec<i64> = (0..rows).map(|_| ts).collect();
        let levels: Vec<Option<&str>> = (0..rows).map(|_| Some("info")).collect();
        let routes: Vec<Option<&str>> = (0..rows).map(|_| None).collect();
        let messages: Vec<Option<String>> = (0..rows)
            .map(|j| Some(format!("file{}-row{}", file_idx, j)))
            .collect();
        let attrs: Vec<Option<&str>> = (0..rows).map(|_| None).collect();

        let msg_array: StringArray = messages.iter().map(|m| m.as_deref()).collect();

        RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(StringArray::from(ids)),
                Arc::new(StringArray::from(services)),
                Arc::new(TimestampMicrosecondArray::from(timestamps)),
                Arc::new(StringArray::from(levels)),
                Arc::new(StringArray::from(routes)),
                Arc::new(msg_array),
                Arc::new(StringArray::from(attrs)),
            ],
        )
        .expect("build small batch")
    }

    fn write_files(partition_dir: &Path, count: usize, rows_per_file: usize, service: &str) {
        fs::create_dir_all(partition_dir).expect("create partition dir");
        for i in 0..count {
            let batch = make_small_batch(service, i, rows_per_file);
            write_parquet_atomic(partition_dir, &batch).expect("write parquet");
        }
    }

    fn count_parquet_files(dir: &Path) -> usize {
        if !dir.exists() {
            return 0;
        }
        fs::read_dir(dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map_or(false, |n| n.ends_with(".parquet"))
            })
            .count()
    }

    fn read_all_rows_from_partition(dir: &Path) -> Vec<String> {
        let files = get_source_files(dir).expect("get sources");
        let mut rows = Vec::new();
        for f in &files {
            let file = File::open(f).expect("open");
            let reader = ParquetRecordBatchReaderBuilder::try_new(file)
                .expect("build reader")
                .build()
                .expect("build record reader");
            for batch_result in reader {
                let batch = batch_result.expect("read batch");
                let col = batch
                    .column(5)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("message column");
                for i in 0..col.len() {
                    rows.push(col.value(i).to_string());
                }
            }
        }
        rows
    }

    fn all_parquet_files(dir: &Path) -> Vec<PathBuf> {
        if !dir.exists() {
            return vec![];
        }
        let mut files: Vec<PathBuf> = fs::read_dir(dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map_or(false, |n| n.ends_with(".parquet"))
            })
            .collect();
        files.sort();
        files
    }

    // ------------------------------------------------------------------
    // Test 1: Basic merge correctness
    // ------------------------------------------------------------------
    #[test]
    fn test_basic_merge() {
        let dir = test_dir("basic_merge");
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=test")
            .join("date=2024-01-01");
        write_files(&part_dir, 3, 5, "test");

        assert_eq!(count_parquet_files(&part_dir), 3);

        let result = compact_partition(&part_dir, DEFAULT_TARGET_SIZE, false)
            .expect("compact");
        assert_eq!(result.files_compacted, 3);

        // After compaction: only the merged file remains.
        let remaining = count_parquet_files(&part_dir);
        assert_eq!(remaining, 1, "should have exactly one merged file");

        // Verify all 15 rows are present.
        let rows = read_all_rows_from_partition(&part_dir);
        assert_eq!(rows.len(), 15);
        assert!(rows.contains(&"file0-row0".to_string()));
        assert!(rows.contains(&"file2-row4".to_string()));

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 2: Manifest written before source deletion
    // ------------------------------------------------------------------
    #[test]
    fn test_manifest_before_deletion() {
        let dir = test_dir("manifest_order");
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=test")
            .join("date=2024-01-01");
        write_files(&part_dir, 2, 3, "test");

        let sources = get_source_files(&part_dir).expect("get sources");
        assert_eq!(sources.len(), 2);

        // Phase 1: merge and write manifest, but do NOT delete sources.
        let manifest = compact_phase1(&part_dir, &sources).expect("phase1");

        // The manifest must exist now, before any source file was deleted.
        let manifest_path = part_dir.join(MANIFEST_FILE);
        assert!(manifest_path.exists(), "manifest must exist after phase1");

        // Verify manifest content.
        let manifest_data = fs::read_to_string(&manifest_path).expect("read manifest");
        let parsed: Manifest = serde_json::from_str(&manifest_data).expect("parse manifest");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].status, "pending_delete");
        assert_eq!(parsed.entries[0].source_files.len(), 2);

        // Source files must still exist at this point.
        for src in &manifest.source_files {
            assert!(src.exists(), "source file must still exist before phase2");
        }

        // The merged file must exist.
        assert!(
            manifest.merged_file.exists(),
            "merged file must exist after phase1"
        );

        // Phase 2: delete sources and mark completed.
        compact_phase2(&part_dir, &manifest).expect("phase2");

        // Now the source files should be gone.
        for src in &manifest.source_files {
            assert!(!src.exists(), "source file must be deleted after phase2");
        }

        // Manifest should now be "completed".
        let manifest_data = fs::read_to_string(&manifest_path).expect("read manifest");
        let parsed: Manifest = serde_json::from_str(&manifest_data).expect("parse manifest");
        assert_eq!(parsed.entries[0].status, "completed");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 3: Crash between merge-write and deletion
    // ------------------------------------------------------------------
    #[test]
    fn test_crash_between_merge_and_deletion() {
        let dir = test_dir("crash_recovery");
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=test")
            .join("date=2024-01-01");
        write_files(&part_dir, 3, 2, "test");

        let sources = get_source_files(&part_dir).expect("get sources");
        let manifest = compact_phase1(&part_dir, &sources).expect("phase1");

        // Simulate crash: some source files get deleted by phase2, some remain.
        // We manually delete only one source file to simulate partial crash.
        let half = manifest.source_files.len() / 2;
        for src in manifest.source_files.iter().take(half) {
            if src.exists() {
                fs::remove_file(src).expect("remove source");
            }
        }

        // Now run startup reconciliation.
        reconcile_compactions(&dir).expect("reconcile");

        // After reconciliation:
        // - All source files (remaining ones) should be gone.
        for src in &manifest.source_files {
            assert!(!src.exists(), "all sources must be deleted after reconcile");
        }
        // - The merged file should still exist.
        assert!(
            manifest.merged_file.exists(),
            "merged file must survive reconcile"
        );
        // - Only one parquet file remains (the merged one).
        assert_eq!(count_parquet_files(&part_dir), 1);

        // Verify we have all rows.
        let rows = read_all_rows_from_partition(&part_dir);
        assert_eq!(rows.len(), 6);

        // Manifest entry should be "completed".
        let manifest_path = part_dir.join(MANIFEST_FILE);
        let data = fs::read_to_string(&manifest_path).expect("read manifest");
        let parsed: Manifest = serde_json::from_str(&data).expect("parse manifest");
        assert_eq!(parsed.entries[0].status, "completed");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 4: Reconciliation is idempotent
    // ------------------------------------------------------------------
    #[test]
    fn test_reconciliation_idempotent() {
        let dir = test_dir("idempotent");
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=test")
            .join("date=2024-01-01");
        write_files(&part_dir, 2, 3, "test");

        let sources = get_source_files(&part_dir).expect("get sources");
        let manifest = compact_phase1(&part_dir, &sources).expect("phase1");

        // Simulate crash: delete nothing, just leave it "pending_delete".
        // Run reconcile first time.
        reconcile_compactions(&dir).expect("reconcile first");

        // All sources should be gone.
        for src in &manifest.source_files {
            assert!(!src.exists(), "source must be deleted after first reconcile");
        }
        assert!(manifest.merged_file.exists());

        // Run reconcile second time — must not error.
        reconcile_compactions(&dir).expect("reconcile second");

        // State must be unchanged.
        assert!(manifest.merged_file.exists());
        assert_eq!(count_parquet_files(&part_dir), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 5: Small partition (below target size) still compacts
    // ------------------------------------------------------------------
    #[test]
    fn test_small_partition() {
        let dir = test_dir("small_partition");
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=test")
            .join("date=2024-01-01");
        // Only 2 tiny files — far from 128 MB.
        write_files(&part_dir, 2, 1, "test");

        let result = compact_partition(&part_dir, DEFAULT_TARGET_SIZE, false)
            .expect("compact");
        assert_eq!(result.files_compacted, 2);

        assert_eq!(count_parquet_files(&part_dir), 1);

        // Verify both rows are in the merged file.
        let rows = read_all_rows_from_partition(&part_dir);
        assert_eq!(rows.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 6: Today's partition is not compacted
    // ------------------------------------------------------------------
    #[test]
    fn test_skip_today_partition() {
        let dir = test_dir("skip_today");
        let today = today_date_string();
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=test")
            .join(format!("date={}", today));
        write_files(&part_dir, 3, 2, "test");

        assert_eq!(count_parquet_files(&part_dir), 3);

        let result = compact_partition(&part_dir, DEFAULT_TARGET_SIZE, true)
            .expect("compact");
        assert_eq!(result.files_compacted, 0, "should skip today's partition");

        // Files unchanged.
        assert_eq!(count_parquet_files(&part_dir), 3);

        // Same test with skip_today=false should compact it.
        let result2 = compact_partition(&part_dir, DEFAULT_TARGET_SIZE, false)
            .expect("compact");
        assert_eq!(result2.files_compacted, 3, "should compact when not skipping");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 7: Crash inside phase1 — after merged‑file write, before
    // manifest status update to pending_delete.
    //
    // With the manifest‑first ordering, the crash window is:
    //   manifest(pending_merge) → merged file → CRASH → manifest(pending_delete)
    //
    // The state after the crash: manifest has a "pending_merge" entry,
    // the merged file exists on disk, and all source files are still
    // present.  Reconciliation must finish the cycle without duplication.
    // ------------------------------------------------------------------
    #[test]
    fn test_crash_after_merge_write_before_manifest_update() {
        let dir = test_dir("crash_after_merge");
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=test")
            .join("date=2024-01-01");
        write_files(&part_dir, 3, 2, "test");

        // Manually reconstruct the state after a crash between step 2
        // and step 3 of compact_phase1 (merged file written, manifest
        // still at "pending_merge").
        let sources = get_source_files(&part_dir).expect("get sources");
        let batches = read_parquet_files(&sources).expect("read");
        let merged = concatenate_batches(&batches).expect("concat");

        let unique_id = uuid::Uuid::new_v4().to_string();
        let merged_name = format!("part-{}.parquet", unique_id);
        let source_names: Vec<String> = sources
            .iter()
            .map(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();

        // Write manifest with pending_merge (step 1).
        add_manifest_entry(
            &part_dir,
            ManifestEntry {
                merged_file: merged_name.clone(),
                source_files: source_names.clone(),
                status: "pending_merge".into(),
            },
        )
        .expect("add manifest entry");

        // Write merged file (step 2 — crash happens right after this).
        let merged_path =
            crate::store::io::write_parquet_atomic_with_id(&part_dir, &unique_id, &merged)
                .expect("write merged");

        // At this point: manifest has "pending_merge", merged file exists,
        // sources still present.  This is the crash state.

        // Run reconciliation — must finish the cycle.
        reconcile_compactions(&dir).expect("reconcile");

        // All source files must be gone.
        assert_eq!(count_parquet_files(&part_dir), 1);
        assert!(merged_path.exists(), "merged file must survive");

        // All rows present exactly once — no duplication.
        let rows = read_all_rows_from_partition(&part_dir);
        assert_eq!(rows.len(), 6, "all 6 rows must be present");

        let mut counts = std::collections::HashMap::new();
        for r in &rows {
            *counts.entry(r.clone()).or_insert(0) += 1;
        }
        for (msg, count) in &counts {
            assert_eq!(*count, 1, "row '{}' must appear exactly once", msg);
        }

        // Manifest entry must be "completed" after reconciliation.
        let manifest_path = part_dir.join(MANIFEST_FILE);
        let data = fs::read_to_string(&manifest_path).expect("read manifest");
        let parsed: Manifest = serde_json::from_str(&data).expect("parse manifest");
        assert_eq!(parsed.entries[0].status, "completed");

        // Run a second compaction pass on the now‑clean partition.
        // Nothing should break and no rows should be duplicated.
        let result = compact_partition(&part_dir, DEFAULT_TARGET_SIZE, false)
            .expect("second compact pass");
        assert_eq!(result.files_compacted, 0, "no files to compact on second pass");
        assert_eq!(count_parquet_files(&part_dir), 1, "still one file");
        let rows = read_all_rows_from_partition(&part_dir);
        assert_eq!(rows.len(), 6, "no duplication after second pass");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 8: Crash inside phase1 — before merged‑file write (only
    // manifest pending_merge exists, no merged file).
    //
    // Reconcile must remove the stale entry and leave everything else
    // untouched.
    // ------------------------------------------------------------------
    #[test]
    fn test_crash_before_merge_write() {
        let dir = test_dir("crash_before_merge");
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=test")
            .join("date=2024-01-01");
        write_files(&part_dir, 2, 3, "test");

        let sources = get_source_files(&part_dir).expect("get sources");
        let source_names: Vec<String> = sources
            .iter()
            .map(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        let phantom_name = "part-never-written.parquet".to_string();

        // Write manifest with pending_merge but the merged file was
        // never written (crash before step 2).
        add_manifest_entry(
            &part_dir,
            ManifestEntry {
                merged_file: phantom_name,
                source_files: source_names,
                status: "pending_merge".into(),
            },
        )
        .expect("add manifest entry");

        // Run reconciliation.
        reconcile_compactions(&dir).expect("reconcile");

        // All original files must still exist.
        assert_eq!(count_parquet_files(&part_dir), 2);

        // Manifest must have no entries (the stale one was removed).
        let manifest_path = part_dir.join(MANIFEST_FILE);
        let data = fs::read_to_string(&manifest_path).expect("read manifest");
        let parsed: Manifest = serde_json::from_str(&data).expect("parse manifest");
        assert!(parsed.entries.is_empty(), "stale entry must be removed");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 9: pending_delete sources are excluded from get_source_files
    // before phase2 runs.
    //
    // This verifies that a compaction cycle in the pending_delete state
    // (merged file written, manifest entry written, sources not yet
    // deleted) does not leak its source files as candidates for a second
    // independent compaction pass.
    // ------------------------------------------------------------------
    #[test]
    fn test_pending_delete_sources_excluded() {
        let dir = test_dir("pending_delete_excluded");
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=test")
            .join("date=2024-01-01");
        write_files(&part_dir, 2, 3, "test");

        // Run compact_phase1, which leaves the manifest in pending_delete
        // state (sources still on disk).
        let sources = get_source_files(&part_dir).expect("get sources");
        let _manifest = compact_phase1(&part_dir, &sources).expect("phase1");

        // At this point the manifest has a pending_delete entry. Verify
        // get_source_files excludes the already-scheduled sources.
        let candidates = get_source_files(&part_dir).expect("get sources after phase1");
        assert!(
            candidates.is_empty(),
            "sources referenced by a pending_delete entry must not be re-selected: got {:?}",
            candidates
        );

        // Running compact_partition directly should be a no-op (nothing
        // to compact — all files are in-flight).
        let result = compact_partition(&part_dir, DEFAULT_TARGET_SIZE, false)
            .expect("compact after phase1 (no phase2)");
        assert_eq!(result.files_compacted, 0, "no files should be compacted");

        // All files still present — nothing was double-compacted.
        assert_eq!(count_parquet_files(&part_dir), 3);

        // After reconcile + phase2, everything resolves correctly and
        // no duplication exists.
        compact_phase2(&part_dir, &_manifest).expect("phase2");
        assert_eq!(count_parquet_files(&part_dir), 1);

        let rows = read_all_rows_from_partition(&part_dir);
        assert_eq!(rows.len(), 6);

        let mut counts = std::collections::HashMap::new();
        for r in &rows {
            *counts.entry(r.clone()).or_insert(0) += 1;
        }
        for (_, count) in &counts {
            assert_eq!(*count, 1, "no row should be duplicated");
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
