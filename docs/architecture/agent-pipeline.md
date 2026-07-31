# Agent Pipeline

> **Status:** ✅ Shipped (v0.1)

This is what makes `greplog dev` feel instant. It's a single Rust binary, precompiled per platform. The agent manages the full ingest → persist → query pipeline with crash safety, dedup, and background compaction.

## Workspace Scoping

When `greplog dev` runs, it drops a socket at `.greplog/greplog.sock` in the root of the project. This allows multiple services in a monorepo to stream to the same agent. If you open a completely separate project and run `greplog dev` there, it spawns a second isolated agent on a different UI port (e.g., 4318). Data is stored under `~/.greplog/<project-hash>/`.

## UDS + TCP Fallback

Listens on a Unix domain socket and a local TCP port (e.g., `127.0.0.1:4318`). Host apps connect via UDS (zero config). Dockerized apps fail over to the TCP port seamlessly to escape the container boundary. All communication uses a length-delimited Protobuf framing (`u32 LE length` + `IngestBatch` message).

## Pipeline

### 1. WAL (Write-Ahead Log)

Every incoming batch is written to `wal/` as framed Protobuf segments **before** the acknowledgment is sent to the SDK. If the agent crashes between ingest and storage, the WAL is replayed on restart:

- **WAL format:** 4-byte LE frame length + Protobuf `IngestBatch` bytes.
- **Rotation:** WAL segments are rotated after each successful flush. Completed segments are truncated from the head.
- **Recovery:** On boot, the agent scans `wal/` for any un-flushed segments, re-ingests them through dedup, and pushes them into the Arrow buffer. This guarantees **zero data loss** on crash.

### 2. Dedup

Content-addressed dedup using BLAKE3 hashes of serialized `LogEvent` bytes:

- Each event's hash is looked up in a memory index (`HashMap<[u8; 32], ()>`).
- First-seen events are added to the index and passed to the buffer.
- Duplicates (from SDK retries, network jitter) are silently dropped.
- The dedup index is periodically flushed to disk (`dedup.idx`) so it survives agent restarts within a session window.

### 3. Arrow Buffer

A single-threaded writer accumulates deduplicated events as [Arrow](https://arrow.apache.org/) record batches in memory:

- Events are stored column-wise in a 14-column schema: `id` (utf8), `service` (utf8), `timestamp` (Timestamp<Microsecond>), `level` (utf8), `route` (utf8), `message` (utf8), `attributes` (utf8 — JSON blob), `logger_name` (utf8), `file` (utf8), `line` (Int32), `correlation_id` (utf8), `stack_trace` (List<utf8>), `exception_type` (utf8), `exception_message` (utf8).
- `service` and `date` are Hive-style partition columns (derived from directory paths), not stored in the file — DataFusion re-adds them at query time.
- Arrow's columnar layout enables zero-copy filtering in the query engine (no deserialization needed).
- The buffer periodically yields its accumulated batch for flushing (every 2 seconds or when it exceeds a configurable row count).

### 4. Parquet Flush

Every 2 seconds (configurable via `flush_interval_ms`), the buffer flushes to a new Parquet file:

- **Partition layout:** `logs/<date>/<hour>/<seq>.parquet` (e.g., `logs/2026-07-22/13/0000000003.parquet`).
- **Partition key:** The flush timestamp, truncated to the hour.
- **Schema:** Mirrors the Arrow buffer schema, written via `parquet` crate's `ArrowWriter`.
- **WAL truncation:** After a successful flush, the corresponding WAL segments are truncated to reclaim space.
- **Concurrent reads:** Flush writes to a new file atomically (write to `.tmp`, rename). In-flight queries from the query engine see a consistent view via file-system snapshots.

### 5. Compaction

A background compaction scheduler runs on a configurable interval (default: every 5 minutes):

- **Trigger:** Scheduled via `tokio::spawn` with an interval timer.
- **Scope:** Reads all Parquet files in `logs/`, groups them by partition (date+hour).
- **Merge candidates:** Files smaller than `max_file_size` (default 64 MiB) are candidates. If a partition has 2+ small files, they are merged into one.
- **Merge process:** Opens a `CompactionReader` that streams sorted rows from all source files, writes to a temporary Parquet file, then atomically replaces originals.
- **Skip today:** Compaction skips the current hour's partition to avoid contention with active flushes.
- **Idempotency:** Each compaction run records its output manifest; if interrupted, partial merges are discarded (temporary files cleaned up on next boot).

## Query Engine

The dashboard doesn't talk to Parquet directly. The agent exposes a small HTTP query API (`POST /query`) served by the `query_engine` module:

- **Data sources:** Reads from both the in-memory Arrow buffer (unflushed events) and on-disk Parquet files via the `parquet` crate's `SerializedFileReader`.
- **Filter pushdown:** Filters (service name, level, time range) are pushed down to Parquet row-group metadata where possible.
- **Aggregations:** Latency percentiles and error rates are computed by scanning the relevant row groups.

## Crash Recovery Sequence

On `greplog dev` restart:

1. Scan `wal/` for un-flushed segments.
2. Replay each segment through dedup → buffer.
3. Trigger an immediate flush to persist replayed events.
4. Start normal ingest. This is transparent to SDKs — they reconnect via UDS and continue streaming.

## File Layout

```
~/.greplog/<project-hash>/
├── wal/
│   ├── 0000000001.wal
│   └── 0000000002.wal
├── dedup.idx
├── logs/
│   └── 2026-07-22/
│       ├── 13/
│       │   ├── 0000000003.parquet
│       │   └── 0000000005.parquet
│       └── 14/
│           └── 0000000007.parquet
└── greplog.db                    # metadata only (partition manifests, config)
```
