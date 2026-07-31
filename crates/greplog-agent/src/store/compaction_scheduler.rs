use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Run one compaction cycle on all eligible partitions.
///
/// Discovers every `date=YYYY-MM-DD` leaf directory under `data_dir/data/`
/// and calls [`compact_partition`] on each with `skip_today=true` so the
/// actively‑written partition is never touched.
///
/// Returns the number of partitions that were actually compacted
/// (i.e. had ≥2 source files merged).
pub fn compact_eligible_partitions(data_dir: &Path) -> Result<usize> {
    let data = data_dir.join("data");
    if !data.exists() {
        return Ok(0);
    }
    let partitions = super::compaction::find_partition_dirs(&data);
    let mut compacted = 0;
    for dir in &partitions {
        match super::compaction::compact_partition(
            dir,
            super::compaction::DEFAULT_TARGET_SIZE,
            true, // skip_today – never compact the partition being written to
        ) {
            Ok(result) => {
                if result.files_compacted > 0 {
                    tracing::info!(
                        "Compacted {:?}: {} files merged into 1",
                        dir,
                        result.files_compacted,
                    );
                    compacted += 1;
                }
            }
            Err(e) => {
                tracing::warn!("Compaction failed for {:?}: {} — skipping", dir, e);
            }
        }
    }
    Ok(compacted)
}

/// Spawn a background task that periodically calls
/// [`compact_eligible_partitions`] on the given `data_dir`.
///
/// The first tick is skipped (the agent has just started and may still be
/// recovering); subsequent ticks run compaction inside `spawn_blocking`
/// so they never block the tokio runtime.
pub fn spawn_compaction_scheduler(
    data_dir: PathBuf,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        // Skip the first immediate tick — no point compacting at startup.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            let dir = data_dir.clone();
            if let Err(e) = tokio::task::spawn_blocking(move || compact_eligible_partitions(&dir))
                .await
                .unwrap_or_else(|e| Err(anyhow::anyhow!("spawn_blocking join: {e}")))
            {
                tracing::error!("Compaction cycle failed: {e}");
            }
        }
    })
}

#[cfg(test)]
#[path = "../../tests/modules/store/compaction_scheduler.rs"]
mod tests;
