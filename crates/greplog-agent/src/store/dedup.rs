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
        files.sort_by_key(|b| std::cmp::Reverse(b.1));

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

    #[allow(dead_code)]
    pub fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Filter an `IngestBatch`, returning a new batch that only contains
    /// events whose content IDs are NOT in the cache.
    ///
    /// Uses the pre-populated `event_id` from the proto (assigned at ingest
    /// time by `handle_batch()`). For events with an empty `event_id` (old
    /// WAL records from before the schema change) falls back to computing
    /// the content hash.
    ///
    /// New (non-duplicate) events are **not** added to the cache — that's
    /// done when they are flushed.
    pub fn filter_batch(&self, batch: &IngestBatch) -> IngestBatch {
        IngestBatch {
            service_name: batch.service_name.clone(),
            instance_id: batch.instance_id.clone(),
            batch_seq: batch.batch_seq,
            logs: batch
                .logs
                .iter()
                .filter(|l| {
                    let key = if l.event_id.is_empty() {
                        crate::store::flush::log_content_id(l)
                    } else {
                        l.event_id.clone()
                    };
                    !self.ids.contains(&key)
                })
                .cloned()
                .collect(),
            spans: batch
                .spans
                .iter()
                .filter(|s| {
                    let key = if s.event_id.is_empty() {
                        crate::store::flush::span_content_id(s)
                    } else {
                        s.event_id.clone()
                    };
                    !self.ids.contains(&key)
                })
                .cloned()
                .collect(),
            metrics: batch
                .metrics
                .iter()
                .filter(|m| {
                    let key = if m.event_id.is_empty() {
                        crate::store::flush::metric_content_id(m)
                    } else {
                        m.event_id.clone()
                    };
                    !self.ids.contains(&key)
                })
                .cloned()
                .collect(),
        }
    }

    /// Insert IDs from an IngestBatch (after it has been flushed).
    pub fn insert_batch(&mut self, batch: &IngestBatch) {
        for log in &batch.logs {
            let id = if log.event_id.is_empty() {
                crate::store::flush::log_content_id(log)
            } else {
                log.event_id.clone()
            };
            self.ids.insert(id);
        }
        for span in &batch.spans {
            let id = if span.event_id.is_empty() {
                crate::store::flush::span_content_id(span)
            } else {
                span.event_id.clone()
            };
            self.ids.insert(id);
        }
        for metric in &batch.metrics {
            let id = if metric.event_id.is_empty() {
                crate::store::flush::metric_content_id(metric)
            } else {
                metric.event_id.clone()
            };
            self.ids.insert(id);
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
        } else if path.extension().is_some_and(|e| e == "parquet") {
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
#[path = "../../tests/modules/store/dedup.rs"]
mod tests;
