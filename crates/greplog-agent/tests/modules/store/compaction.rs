use super::*;
use crate::store::io::write_parquet_atomic;
use arrow::array::*;
use arrow::record_batch::RecordBatch;
use greplog_core::arrow_schema;
use std::sync::Arc;

#[allow(dead_code)]
fn make_log_batch(service: &str, message: &str) -> RecordBatch {
    let schema = arrow_schema::log_schema();
    let ts = 1_700_000_000_000_000;
    let mut st_builder = ListBuilder::new(StringBuilder::new());
    st_builder.append(false);
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
            Arc::new(StringArray::from(vec![None::<&str>])),
            Arc::new(StringArray::from(vec![None::<&str>])),
            Arc::new(Int32Array::from(vec![None::<i32>])),
            Arc::new(StringArray::from(vec![None::<&str>])),
            Arc::new(st_builder.finish()),
            Arc::new(StringArray::from(vec![None::<&str>])),
            Arc::new(StringArray::from(vec![None::<&str>])),
        ],
    )
    .expect("build log batch")
}

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
    let none_str: Vec<Option<&str>> = (0..rows).map(|_| None).collect();

    let msg_array: StringArray = messages.iter().map(|m| m.as_deref()).collect();
    let mut st_builder = ListBuilder::new(StringBuilder::new());
    for _ in 0..rows {
        st_builder.append(false);
    }

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
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(Int32Array::from(vec![None::<i32>; rows])),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(st_builder.finish()),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(StringArray::from(none_str)),
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
                .is_some_and(|n| n.ends_with(".parquet"))
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

#[allow(dead_code)]
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
                .is_some_and(|n| n.ends_with(".parquet"))
        })
        .collect();
    files.sort();
    files
}

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

    let remaining = count_parquet_files(&part_dir);
    assert_eq!(remaining, 1, "should have exactly one merged file");

    let rows = read_all_rows_from_partition(&part_dir);
    assert_eq!(rows.len(), 15);
    assert!(rows.contains(&"file0-row0".to_string()));
    assert!(rows.contains(&"file2-row4".to_string()));

    let _ = fs::remove_dir_all(&dir);
}

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

    let manifest = compact_phase1(&part_dir, &sources).expect("phase1");

    let manifest_path = part_dir.join(MANIFEST_FILE);
    assert!(manifest_path.exists(), "manifest must exist after phase1");

    let manifest_data = fs::read_to_string(&manifest_path).expect("read manifest");
    let parsed: Manifest = serde_json::from_str(&manifest_data).expect("parse manifest");
    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.entries[0].status, "pending_delete");
    assert_eq!(parsed.entries[0].source_files.len(), 2);

    for src in &manifest.source_files {
        assert!(src.exists(), "source file must still exist before phase2");
    }

    assert!(
        manifest.merged_file.exists(),
        "merged file must exist after phase1"
    );

    compact_phase2(&part_dir, &manifest).expect("phase2");

    for src in &manifest.source_files {
        assert!(!src.exists(), "source file must be deleted after phase2");
    }

    let manifest_data = fs::read_to_string(&manifest_path).expect("read manifest");
    let parsed: Manifest = serde_json::from_str(&manifest_data).expect("parse manifest");
    assert_eq!(parsed.entries[0].status, "completed");

    let _ = fs::remove_dir_all(&dir);
}

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

    let half = manifest.source_files.len() / 2;
    for src in manifest.source_files.iter().take(half) {
        if src.exists() {
            fs::remove_file(src).expect("remove source");
        }
    }

    reconcile_compactions(&dir).expect("reconcile");

    for src in &manifest.source_files {
        assert!(!src.exists(), "all sources must be deleted after reconcile");
    }
    assert!(
        manifest.merged_file.exists(),
        "merged file must survive reconcile"
    );
    assert_eq!(count_parquet_files(&part_dir), 1);

    let rows = read_all_rows_from_partition(&part_dir);
    assert_eq!(rows.len(), 6);

    let manifest_path = part_dir.join(MANIFEST_FILE);
    let data = fs::read_to_string(&manifest_path).expect("read manifest");
    let parsed: Manifest = serde_json::from_str(&data).expect("parse manifest");
    assert_eq!(parsed.entries[0].status, "completed");

    let _ = fs::remove_dir_all(&dir);
}

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

    reconcile_compactions(&dir).expect("reconcile first");

    for src in &manifest.source_files {
        assert!(!src.exists(), "source must be deleted after first reconcile");
    }
    assert!(manifest.merged_file.exists());

    reconcile_compactions(&dir).expect("reconcile second");

    assert!(manifest.merged_file.exists());
    assert_eq!(count_parquet_files(&part_dir), 1);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_small_partition() {
    let dir = test_dir("small_partition");
    let part_dir = dir
        .join("data")
        .join("table_type=logs")
        .join("service=test")
        .join("date=2024-01-01");
    write_files(&part_dir, 2, 1, "test");

    let result = compact_partition(&part_dir, DEFAULT_TARGET_SIZE, false)
        .expect("compact");
    assert_eq!(result.files_compacted, 2);

    assert_eq!(count_parquet_files(&part_dir), 1);

    let rows = read_all_rows_from_partition(&part_dir);
    assert_eq!(rows.len(), 2);

    let _ = fs::remove_dir_all(&dir);
}

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

    assert_eq!(count_parquet_files(&part_dir), 3);

    let result2 = compact_partition(&part_dir, DEFAULT_TARGET_SIZE, false)
        .expect("compact");
    assert_eq!(result2.files_compacted, 3, "should compact when not skipping");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_crash_after_merge_write_before_manifest_update() {
    let dir = test_dir("crash_after_merge");
    let part_dir = dir
        .join("data")
        .join("table_type=logs")
        .join("service=test")
        .join("date=2024-01-01");
    write_files(&part_dir, 3, 2, "test");

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

    add_manifest_entry(
        &part_dir,
        ManifestEntry {
            merged_file: merged_name.clone(),
            source_files: source_names.clone(),
            status: "pending_merge".into(),
        },
    )
    .expect("add manifest entry");

    let merged_path =
        crate::store::io::write_parquet_atomic_with_id(&part_dir, &unique_id, &merged)
            .expect("write merged");

    reconcile_compactions(&dir).expect("reconcile");

    assert_eq!(count_parquet_files(&part_dir), 1);
    assert!(merged_path.exists(), "merged file must survive");

    let rows = read_all_rows_from_partition(&part_dir);
    assert_eq!(rows.len(), 6, "all 6 rows must be present");

    let mut counts = std::collections::HashMap::new();
    for r in &rows {
        *counts.entry(r.clone()).or_insert(0) += 1;
    }
    for (msg, count) in &counts {
        assert_eq!(*count, 1, "row '{}' must appear exactly once", msg);
    }

    let manifest_path = part_dir.join(MANIFEST_FILE);
    let data = fs::read_to_string(&manifest_path).expect("read manifest");
    let parsed: Manifest = serde_json::from_str(&data).expect("parse manifest");
    assert_eq!(parsed.entries[0].status, "completed");

    let result = compact_partition(&part_dir, DEFAULT_TARGET_SIZE, false)
        .expect("second compact pass");
    assert_eq!(result.files_compacted, 0, "no files to compact on second pass");
    assert_eq!(count_parquet_files(&part_dir), 1, "still one file");
    let rows = read_all_rows_from_partition(&part_dir);
    assert_eq!(rows.len(), 6, "no duplication after second pass");

    let _ = fs::remove_dir_all(&dir);
}

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

    add_manifest_entry(
        &part_dir,
        ManifestEntry {
            merged_file: phantom_name,
            source_files: source_names,
            status: "pending_merge".into(),
        },
    )
    .expect("add manifest entry");

    reconcile_compactions(&dir).expect("reconcile");

    assert_eq!(count_parquet_files(&part_dir), 2);

    let manifest_path = part_dir.join(MANIFEST_FILE);
    let data = fs::read_to_string(&manifest_path).expect("read manifest");
    let parsed: Manifest = serde_json::from_str(&data).expect("parse manifest");
    assert!(parsed.entries.is_empty(), "stale entry must be removed");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_pending_delete_sources_excluded() {
    let dir = test_dir("pending_delete_excluded");
    let part_dir = dir
        .join("data")
        .join("table_type=logs")
        .join("service=test")
        .join("date=2024-01-01");
    write_files(&part_dir, 2, 3, "test");

    let sources = get_source_files(&part_dir).expect("get sources");
    let _manifest = compact_phase1(&part_dir, &sources).expect("phase1");

    let candidates = get_source_files(&part_dir).expect("get sources after phase1");
    assert!(
        candidates.is_empty(),
        "sources referenced by a pending_delete entry must not be re-selected: got {:?}",
        candidates
    );

    let result = compact_partition(&part_dir, DEFAULT_TARGET_SIZE, false)
        .expect("compact after phase1 (no phase2)");
    assert_eq!(result.files_compacted, 0, "no files should be compacted");

    assert_eq!(count_parquet_files(&part_dir), 3);

    compact_phase2(&part_dir, &_manifest).expect("phase2");
    assert_eq!(count_parquet_files(&part_dir), 1);

    let rows = read_all_rows_from_partition(&part_dir);
    assert_eq!(rows.len(), 6);

    let mut counts = std::collections::HashMap::new();
    for r in &rows {
        *counts.entry(r.clone()).or_insert(0) += 1;
    }
    for count in counts.values() {
        assert_eq!(*count, 1, "no row should be duplicated");
    }

    let _ = fs::remove_dir_all(&dir);
}
