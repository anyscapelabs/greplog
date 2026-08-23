# Greplog: Complete System Architecture Specification

This document details the internal mechanics of Greplog. To achieve the conflicting goals of **high throughput (50k+ logs/sec)**, **zero data loss**, and **sub-second analytical queries**, Greplog strictly isolates network I/O, disk I/O, and CPU-bound analytical processing across different thread models.

## 1. The Threading & Concurrency Model

Greplog avoids `Mutex` locks in the ingestion hot-path entirely. The architecture relies on the "Actor Model" pattern, passing data via lock-free channels across three distinct execution domains:

| Domain | Runtime / Thread Type | Responsibility |
| --- | --- | --- |
| **Network Layer** | Tokio Async Web Workers | Accepting HTTP requests on Port 5050 and 3000. Managing SSE streams. |
| **State Layer** | 2 Dedicated OS Threads | The **WAL Worker** and **MemTable Worker**. They run infinite loops outside of Tokio to prevent async runtime blocking. |
| **Heavy CPU/Disk** | Tokio `spawn_blocking` pool + dedicated query runtime | Parquet compression, background file compaction, and result JSON serialization run on `spawn_blocking`; each query executes on a small dedicated multi-thread Tokio runtime (`worker_threads(2)`) that can be torn down on timeout. |

## 2. The Network Layer (Dual-Port Routing)

To provide maximum security and routing efficiency, the single `greplog dev` binary spins up two completely isolated Axum servers running concurrently via `tokio::join!`.

- **Port 5050 (Ingest API):** Exposes only `POST /api/log`. Can be safely exposed to the open internet or internal VPC. Configured to accept larger JSON payload batches (e.g., 5MB limit).
- **Port 3000 (Dashboard & Admin):** Exposes `POST /api/search` (structured, partition-pruned queries), `POST /api/query` (raw-SQL escape hatch), `GET /api/stats` (disk usage), `GET /api/tail` (SSE), and serves the embedded frontend via `rust-embed`. Intended to be secured behind a firewall or accessed via localhost/SSH tunnel.

## 3. The Zero-Loss Ingestion Pipeline (The Write Path)

This is the exact lifecycle of a log from the moment the SDK sends it to the moment it is queryable.

1. **SDK Client Batching:** The SDK accumulates logs in memory. Every 500 ms (or once 100 logs are queued, whichever comes first), it sends an HTTP POST array to `0.0.0.0:5050/api/log`.
2. **The HTTP Receiver:** Axum parses the JSON batch. It creates a `tokio::sync::oneshot` channel. It bundles the batch and the `oneshot::Sender` together, pushes them into the **Ingest MPSC Channel** (`tokio::sync::mpsc`), and `.await`s the response.
3. **The WAL Worker (OS Thread 1):**
   - Pulls the batch from the MPSC channel.
   - Serializes it to binary (using `bincode` for speed) and appends it to `current.wal` using a `BufWriter`.
   - Executes `file.sync_all()` to force the OS to flush the disk cache to physical storage.
   - Fires the `oneshot` channel with a success signal.
4. **The Network Acknowledgment:** The Axum worker wakes up and instantly returns a `200 OK` to the SDK. Data is now 100% safe.
5. **State Handoff:** The WAL Worker pushes the batch into a second, lock-free crossbeam channel connected to the MemTable Worker.
6. **The MemTable Worker (OS Thread 2):**
   - Reads the batch and appends it to the active Apache Arrow `RecordBatch` in RAM.
   - Takes a clone of the batch and broadcasts it to the `tokio::sync::broadcast` channel to feed the Live Tailing SSE endpoint on Port 3000.

## 4. The Dual-Tier Storage Engine

Greplog uses a two-stage Write-Once-Read-Many (WORM) storage architecture to bridge the gap between real-time RAM limits and the Parquet "small files problem."

### Tier 1: Real-Time Flush (Row Threshold + Periodic Interval)

The MemTable Worker drains durable records into Arrow batches staged in the shared live buffer.

- Once **10,000 rows** (`flush_row_limit`) are staged, the buffered batches are concatenated and written to Parquet.
- For low-traffic deployments that never reach the row threshold, a flush also fires **10 seconds** (`flush_interval_secs`) after the last flush while any rows are pending.

- It compresses the Arrow data using Snappy.
- It writes a file to the active partition: `data/logs/year=2026/month=08/day=09/service=auth-api/chunk_{nanos}.parquet`.
- Once the file is on disk, it signals the WAL Worker to confirm the written rows, sealing and reclaiming the corresponding WAL segments.

### Tier 2: Background Compaction (Interval × File-Count Trigger)

If Greplog runs for weeks, Tier 1 creates thousands of small files, which slows down query planning.

- A background timer ticks every **3600 seconds** (`compaction_run_interval_secs`).
- It scans partitioned directories for leaf partitions holding more than **5 Parquet chunks** (`max_files_before_compaction`).
- It merges each crowded partition's chunks, streamed batch-by-batch, into a single highly compressed `compacted_{uuid}.parquet` file using Zstd.
- It performs an atomic file system swap to replace the small files with the compacted file.

## 5. The Query Engine (Apache DataFusion)

The read path merges historical disk data and live memory data invisibly to the user.

1. **Session Context Initialization:** On startup, Greplog registers the `data/logs/` tree as a `ListingTable` (`parquet_logs`) and the shared live buffer as a live table provider (`live_logs`).
2. **The Unified View:** A SQL view `logs` is created once at boot that `UNION ALL`s the two tiers — the Parquet `ListingTable` and the live buffer provider — bridging the schema gap by deriving the `year`/`month`/`day` partition columns from `timestamp_us` on the live side.
3. **Execution:** When Port 3000 receives a SQL query (e.g., `SELECT count(*) FROM logs WHERE level = 'ERROR'`), it runs against the `logs` view:
   - It scans the Parquet `ListingTable` (Partition pruned, Column pruned).
   - It scans the live buffer provider, which snapshots the shared `LiveBuffer` under a brief read lock on every scan.
   - DataFusion plans and executes the query, applying any timeout and row cap; the result is serialized to JSON on a blocking thread and returned to the caller.

## 6. Schema Definition

To maximize columnar compression and query speed, the Arrow schema is rigidly typed. (Flexible JSON payloads are stored in a dedicated nullable `raw_body` column).

| Column | Arrow Type | Description |
| --- | --- | --- |
| `timestamp_us` | `Timestamp(Microsecond)` | The primary time-series axis for partition pruning. |
| `trace_id` | `Utf8` (nullable) | Correlation id grouping logs from one job or HTTP request. |
| `level` | `Dictionary(Int16, Utf8)` | Severity level (INFO, WARN, ERROR). Dictionary encoding saves space. |
| `service` | `Dictionary(Int16, Utf8)` | The source application (e.g., "auth-api", "frontend"). Stored only in the `service=<name>` partition folder, never in the file. |
| `message` | `Utf8` | The raw log string. |
| `raw_body` | `Utf8` (nullable) | Stringified JSON payload (request body, stack trace, worker data). |

## 7. Automated Maintenance

### Crash Recovery

If power is lost, Greplog recovers state autonomously:

1. On boot, before Axum starts, Greplog replays `current.wal` (sealed segments then the active file).
2. It deserializes the `bincode` logs and stages them into the live buffer so they are immediately queryable.
3. The replayed rows are flushed to Parquet on the next flush trigger like any other rows.
4. It starts the servers.

### Auto-Retention (TTL)

Because data is strictly partitioned by time directories on disk, deleting old data requires zero CPU or SQL overhead. A background thread walks the tree for `day=` partitions older than the CLI `--retention-days` flag and issues a standard `fs::remove_dir_all()` command to delete each expired day directory (e.g., `rm -rf data/logs/year=2026/month=07/day=01`).
