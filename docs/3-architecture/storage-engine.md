# Storage Engine

Greplog stores logs as Apache Arrow `RecordBatch`s in memory and Apache Parquet files on disk, with a two-tier compaction strategy.

```
Arrow MemTable ──(Real-time Flusher)──▶ 10 MB Parquet chunks
                ──(Background Compactor)▶ 512 MB Parquet chunks
```

## Tier 1: Arrow MemTable

Committed logs enter an in-memory Apache Arrow `RecordBatch` immediately. This gives:

- **Instant queryability** — in-memory data is queryable by DataFusion with zero disk I/O.
- **Live tail** — the SSE stream reads straight from the MemTable for sub-millisecond fanout to connected dashboards.

The MemTable is bounded; once it reaches a threshold it is handed to the flusher.

## Tier 2: Real-time flusher

The flusher writes memory to **10 MB Parquet chunks every 10 seconds**. Small files land fast, so freshly ingested data is on disk (and durable beyond the WAL) within seconds.

## Tier 3: Background compactor

Once a day, a background compactor merges many small 10 MB files into highly optimized **512 MB Parquet chunks** with full page indexing. Larger files mean:

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