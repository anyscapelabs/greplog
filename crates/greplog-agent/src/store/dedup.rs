use anyhow::Result;
use greplog_core::gen::IngestBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// In-memory set of content IDs from recently-flushed Parquet files.
/// Seeded at startup to catch duplicates from the flush-checkpoint-race
/// window, then (optionally) kept live during normal operation.
pub struct DedupCache {
    ids: HashSet<String>,
    max_seed: usize,
}

impl DedupCache {
    pub fn new(max_seed: usize) -> Self {
        Self {
            ids: HashSet::with_capacity(max_seed.min(1_000_000)),
            max_seed,
        }
    }

    /// Seed the cache from Parquet files under `base_dir/data/`, reading
    /// the `id` column of the most-recently-modified files until we have
    /// collected up to `max_seed` entries.
    ///
    /// Only files with `.parquet` extension are considered.
    pub fn seed_from_recent_files(&mut self, base_dir: &Path) -> Result<()> {
        let data_dir = base_dir.join("data");
        if !data_dir.exists() {
            return Ok(());
        }
        let mut files: Vec<(PathBuf, SystemTime)> = Vec::new();
        collect_parquet_files(&data_dir, &mut files)?;
        files.sort_by(|a, b| b.1.cmp(&a.1));

        let mut loaded = 0usize;
        for (path, _) in &files {
            if loaded >= self.max_seed {
                break;
            }
            let n = load_ids_from_parquet(path, &mut self.ids, self.max_seed - loaded)?;
            loaded += n;
        }
        Ok(())
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Filter an `IngestBatch`, returning a new batch that only contains
    /// events whose content IDs are NOT in the cache.
    ///
    /// New (non-duplicate) events are **not** added to the cache — that's
    /// done when they are flushed (or when they pass through here during
    /// normal operation if we choose to keep the cache live).
    pub fn filter_batch(&self, batch: &IngestBatch) -> IngestBatch {
        IngestBatch {
            service_name: batch.service_name.clone(),
            instance_id: batch.instance_id.clone(),
            batch_seq: batch.batch_seq,
            logs: batch
                .logs
                .iter()
                .filter(|l| !self.ids.contains(&crate::store::flush::log_content_id(l)))
                .cloned()
                .collect(),
            spans: batch
                .spans
                .iter()
                .filter(|s| !self.ids.contains(&crate::store::flush::span_content_id(s)))
                .cloned()
                .collect(),
            metrics: batch
                .metrics
                .iter()
                .filter(|m| !self.ids.contains(&crate::store::flush::metric_content_id(m)))
                .cloned()
                .collect(),
        }
    }

    /// Insert IDs from an IngestBatch (after it has been flushed).
    pub fn insert_batch(&mut self, batch: &IngestBatch) {
        for log in &batch.logs {
            self.ids.insert(crate::store::flush::log_content_id(log));
        }
        for span in &batch.spans {
            self.ids.insert(crate::store::flush::span_content_id(span));
        }
        for metric in &batch.metrics {
            self.ids.insert(crate::store::flush::metric_content_id(metric));
        }
    }
}

fn load_ids_from_parquet(path: &Path, ids: &mut HashSet<String>, limit: usize) -> Result<usize> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(0),
    };
    let reader = match ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| anyhow::anyhow!("parquet reader: {e}"))
        .and_then(|b| b.build().map_err(|e| anyhow::anyhow!("build reader: {e}")))
    {
        Ok(r) => r,
        Err(_) => return Ok(0),
    };
    let mut count = 0usize;
    for batch_result in reader {
        let batch = match batch_result {
            Ok(b) => b,
            Err(_) => break,
        };
        let idx = match batch.schema().index_of("id") {
            Ok(i) => i,
            Err(_) => continue,
        };
        let col = batch.column(idx);
        let arr = col.as_any().downcast_ref::<arrow::array::StringArray>();
        if let Some(arr) = arr {
            for v in arr.iter().flatten() {
                if count >= limit {
                    return Ok(count);
                }
                ids.insert(v.to_string());
                count += 1;
            }
        }
    }
    Ok(count)
}

fn collect_parquet_files(dir: &Path, out: &mut Vec<(PathBuf, SystemTime)>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_parquet_files(&path, out)?;
        } else if path.extension().map_or(false, |e| e == "parquet") {
            if let Ok(meta) = path.metadata() {
                if let Ok(mtime) = meta.modified() {
                    out.push((path, mtime));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::flush::flush_parquet_only;
    use greplog_core::gen::{LogEvent, Metric, Span};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("greplog_dedup_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn make_log(service: &str, timestamp_ns: i64, level: &str, msg: &str) -> LogEvent {
        LogEvent {
            service_name: service.into(),
            message: msg.into(),
            level: level.into(),
            timestamp_ns,
            logger_name: String::new(),
            file: String::new(),
            line: 0,
            correlation_id: String::new(),
            attributes: HashMap::new(),
            stack_trace: Vec::new(),
            exception_type: String::new(),
            exception_message: String::new(),
        }
    }

    fn make_span(
        service: &str, start_ns: i64, end_ns: i64, name: &str,
    ) -> Span {
        Span {
            service_name: service.into(),
            name: name.into(),
            start_time_ns: start_ns,
            end_time_ns: end_ns,
            correlation_id: String::new(),
            parent_correlation_id: String::new(),
            route: String::new(),
            method: String::new(),
            status_code: 0,
            error: String::new(),
            attributes: HashMap::new(),
            kind: 0,
        }
    }

    fn make_metric(service: &str, timestamp_ns: i64, name: &str, value: f64) -> Metric {
        Metric {
            service_name: service.into(),
            name: name.into(),
            value,
            timestamp_ns,
            labels: HashMap::new(),
            r#type: 1,
            bucket_values: Vec::new(),
        }
    }

    // ------------------------------------------------------------------
    // Test 1: DedupCache is initially empty.
    // ------------------------------------------------------------------
    #[test]
    fn test_empty_cache() {
        let cache = DedupCache::new(1000);
        assert_eq!(cache.len(), 0);
        assert!(!cache.contains("nonexistent"));
    }

    // ------------------------------------------------------------------
    // Test 2: Seed cache from Parquet files on disk.
    // ------------------------------------------------------------------
    #[test]
    fn test_seed_from_parquet() {
        let dir = test_dir("seed");

        // Flush events so they land in Parquet.
        let batch = IngestBatch {
            service_name: "svc".into(),
            instance_id: "i1".into(),
            batch_seq: 1,
            logs: vec![make_log("svc", 1_700_000_000_000_000_000, "info", "seed me")],
            spans: vec![make_span("svc", 1_700_000_000_000_000_000, 1_700_000_000_010_000_000, "op")],
            metrics: vec![make_metric("svc", 1_700_000_000_000_000_000, "cpu", 0.5)],
        };
        flush_parquet_only(&dir, &[batch]).expect("flush to seed");

        let mut cache = DedupCache::new(1000);
        cache.seed_from_recent_files(&dir).expect("seed");

        assert!(cache.contains(&crate::store::flush::log_content_id(&make_log(
            "svc", 1_700_000_000_000_000_000, "info", "seed me",
        ))));

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 3: filter_batch removes duplicate events.
    // ------------------------------------------------------------------
    #[test]
    fn test_filter_batch_dedup() {
        let mut cache = DedupCache::new(1000);

        // Pre-populate with a known ID.
        let known_log = make_log("svc", 1_000_000, "info", "duplicate");
        let id = crate::store::flush::log_content_id(&known_log);
        cache.ids.insert(id.clone());
        assert!(cache.contains(&id));

        let batch = IngestBatch {
            service_name: "svc".into(),
            instance_id: "i1".into(),
            batch_seq: 1,
            logs: vec![
                known_log,
                make_log("svc", 2_000_000, "debug", "unique"),
            ],
            spans: vec![],
            metrics: vec![],
        };

        let filtered = cache.filter_batch(&batch);
        assert_eq!(filtered.logs.len(), 1);
        assert_eq!(filtered.logs[0].level, "debug");
        assert_eq!(filtered.logs[0].timestamp_ns, 2_000_000);
    }

    // ------------------------------------------------------------------
    // Test 4: insert_batch adds IDs to cache.
    // ------------------------------------------------------------------
    #[test]
    fn test_insert_batch_updates_cache() {
        let mut cache = DedupCache::new(1000);
        let log = make_log("svc", 1_000_000, "info", "new");
        let batch = IngestBatch {
            service_name: "svc".into(),
            instance_id: "i1".into(),
            batch_seq: 1,
            logs: vec![log.clone()],
            spans: vec![],
            metrics: vec![],
        };
        cache.insert_batch(&batch);
        let id = crate::store::flush::log_content_id(&log);
        assert!(cache.contains(&id));
    }

    // ------------------------------------------------------------------
    // Test 5: Seed from empty directory.
    // ------------------------------------------------------------------
    #[test]
    fn test_seed_empty_dir() {
        let dir = test_dir("seed_empty");
        let mut cache = DedupCache::new(1000);
        cache.seed_from_recent_files(&dir).expect("seed from empty");
        assert_eq!(cache.len(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 6: Seed respects max_seed limit.
    // ------------------------------------------------------------------
    #[test]
    fn test_seed_respects_limit() {
        let dir = test_dir("seed_limit");

        // Flush enough events to exceed a small limit.
        let logs: Vec<LogEvent> = (0..10)
            .map(|i| make_log("svc", 1_700_000_000_000_000_000 + i as i64, "info", &format!("m{i}")))
            .collect();
        let batch = IngestBatch {
            service_name: "svc".into(),
            instance_id: "i1".into(),
            batch_seq: 1,
            logs,
            spans: vec![],
            metrics: vec![],
        };
        flush_parquet_only(&dir, &[batch]).expect("flush");

        let mut cache = DedupCache::new(3);
        cache.seed_from_recent_files(&dir).expect("seed");
        assert_eq!(cache.len(), 3, "should load at most 3 IDs");

        let _ = fs::remove_dir_all(&dir);
    }
}
