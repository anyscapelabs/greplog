# Storage Engine

Greplog stores logs as Apache Arrow `RecordBatch`s in memory and Apache Parquet files on disk, with a two-tier storage strategy.

```
Arrow MemTable ──(Real-time Flusher)──▶ Parquet chunks (row threshold or 10s interval)
                ──(Background Compactor)▶ single merged chunk per crowded partition
```

## Tier 1: Arrow MemTable

Committed logs enter an in-memory Apache Arrow `RecordBatch` immediately. This gives:

- **Instant queryability** — in-memory data is queryable by DataFusion with zero disk I/O.
- **Live tail** — the SSE stream reads straight from the MemTable for sub-millisecond fanout to connected dashboards.

The MemTable is bounded; once it reaches a threshold it is handed to the flusher.

## Tier 2: Real-time flusher

The flusher writes memory to Hive-partitioned Parquet chunks. A flush fires on either of two triggers, whichever comes first:

- **Row threshold** — once `flush_row_limit` rows (default 10,000) are staged across the buffer.
- **Periodic interval** — once `flush_interval_secs` (default 10) have elapsed since the last flush while rows are pending.

Small files land fast, so freshly ingested data is on disk (and durable beyond the WAL) within seconds — even for low-traffic deployments that never hit the row threshold. Once a chunk is on disk, the WAL worker is signalled to confirm the covered records and seal/reclaim the corresponding WAL segments.

## Tier 3: Background compactor

Every `compaction_run_interval_secs` (default 3600 — hourly) a background sweep walks the partition tree and merges any leaf partition holding more than `max_files_before_compaction` chunks (default 5) into a single highly compressed `compacted_<uuid>.parquet` file. Larger files mean:

- fewer files per query scan,
- better predicate pushdown (page-level min/max skipping for timestamps and levels),
- lower memory pressure during scans.

## Retention (TTL)

Auto-retention purges Parquet directories older than `--retention-days` (default 30). Old files are deleted directly — no SQL `DELETE`, no vacuum, no tombstones. The WAL and MemTable only hold recent data, so most of the working set is already compacted by the time it plateaus.

## Directory layout

```
~/.greplog/
├── current.wal
├── memtable/       # in-memory state, checkpointed on shutdown
└── parquet/
    ├── 2026-08-09/ # per-day directories, purged by TTL
    └── ...
```