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
pub(crate) const DEFAULT_TARGET_SIZE: u64 = 128 * 1024 * 1024;

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

/// Whether a partition should be compacted this cycle, given whether it's
/// the actively‑written (today, UTC) partition, how many merge‑candidate
/// files it currently has, and the configured active‑partition threshold.
///
/// Historical (non‑today) partitions are always eligible here — they only
/// need ≥2 candidate files to actually produce a merge, which is checked
/// separately by the caller. Today's partition is additionally gated:
/// `None` means it is never compacted (preserves the original,
/// unconditional `skip_today` behavior); `Some(threshold)` makes it
/// eligible once `candidate_count >= threshold`.
fn is_eligible_for_compaction(
    is_today: bool,
    candidate_count: usize,
    active_partition_file_threshold: Option<usize>,
) -> bool {
    if !is_today {
        return true;
    }
    match active_partition_file_threshold {
        None => false,
        Some(threshold) => candidate_count >= threshold,
    }
}

/// Compact a single partition directory.
///
/// Reads eligible parquet files (see [`get_source_files`]), merges them
/// into one, writes the merged file atomically, writes a manifest entry,
/// deletes the source files, and marks the entry as completed. Uses the
/// temp‑file → fsync → rename → dir‑fsync pattern for both the merged
/// data file and the manifest.
///
/// `target_size` bounds which files are even considered a merge
/// candidate: any `.parquet` file already at or above `target_size`
/// bytes is treated as sealed and excluded (see [`get_source_files`]).
/// This keeps repeated compaction of a still‑growing partition bounded to
/// its newest small files instead of re‑merging the whole partition's
/// history on every cycle.
///
/// `active_partition_file_threshold` controls whether the partition
/// corresponding to the current UTC date (the one the flush path is
/// actively writing to) is eligible at all — see
/// [`is_eligible_for_compaction`]. Non‑today partitions are unaffected by
/// this parameter.
pub fn compact_partition(
    dir: &Path,
    target_size: u64,
    active_partition_file_threshold: Option<usize>,
) -> Result<CompactionResult> {
    let is_today = is_today_partition(dir);
    let sources = get_source_files(dir, target_size)?;

    if !is_eligible_for_compaction(is_today, sources.len(), active_partition_file_threshold) {
        debug!(
            "Active partition {:?} not yet eligible ({} candidate file(s), threshold {:?}) — skipping this cycle",
            dir,
            sources.len(),
            active_partition_file_threshold,
        );
        return Ok(CompactionResult { files_compacted: 0 });
    }

    if sources.len() < 2 {
        return Ok(CompactionResult { files_compacted: 0 });
    }

    if is_today {
        info!(
            "Active partition {:?} has {} candidate file(s) — compacting",
            dir,
            sources.len(),
        );
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
    let merged_path = crate::store::io::write_parquet_atomic_with_id(dir, &unique_id, &merged)?;

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

    update_manifest_entry_status(dir, &manifest.merged_file, "completed")?;

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
                anyhow::bail!(
                    "Cannot parse manifest {:?}: {} — aborting startup recovery",
                    manifest_path,
                    e,
                );
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
                        info!("Reconciled pending_merge (merged exists) in {:?}", part_dir);
                    } else {
                        // Pre‑registration that never materialised.
                        entry.status = "removed".into();
                        changed = true;
                        info!("Removed pending_merge (merged missing) in {:?}", part_dir);
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
/// (`pending_merge` or `pending_delete`), and are not already sealed (at
/// or above `target_size` bytes).
fn get_source_files(dir: &Path, target_size: u64) -> Result<Vec<PathBuf>> {
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
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_candidate_name = name.ends_with(".parquet")
                && !active_sources.contains(name)
                && !active_merged.contains(name)
                && name != MANIFEST_FILE
                && !name.ends_with(".tmp");
            if !is_candidate_name {
                return false;
            }
            // Files already at/above target_size are treated as sealed and
            // excluded from the merge set. Without this, repeatedly
            // compacting a partition that's still being written to (the
            // active/today partition) would re-read and rewrite its entire
            // accumulated history on every cycle instead of just the
            // newest small files. A file that can't be stat'd is kept as a
            // candidate rather than silently dropped: read_parquet_files
            // will surface a clear error for a genuinely missing/unreadable
            // file instead of it being quietly excluded forever.
            match fs::metadata(p) {
                Ok(meta) => meta.len() < target_size,
                Err(_) => true,
            }
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
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;
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
fn update_manifest_entry_status(dir: &Path, merged_file: &Path, new_status: &str) -> Result<()> {
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
pub(crate) fn find_partition_dirs(data_dir: &Path) -> Vec<PathBuf> {
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
        .is_some_and(|n| n.starts_with("date=") && n.len() == 15)
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
    let dirname = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let date_str = dirname.strip_prefix("date=").unwrap_or("");
    if date_str.is_empty() || date_str.len() != 10 {
        return false;
    }
    date_str == today_date_string()
}

/// Convert days since Unix epoch to `YYYY-MM-DD` using Howard Hinnant's
/// civil‑date algorithm.
pub(crate) fn civil_days_to_ymd(days: i64) -> String {
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
#[path = "../../tests/modules/store/compaction.rs"]
mod tests;
