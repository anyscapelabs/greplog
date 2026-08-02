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

- **Backend:** [DataFusion](https://datafusion.apache.org/) — a full, in-process SQL execution engine on top of Apache Arrow. The query engine is stateless; each request is a fresh planning + execution pass.
- **Data sources:** The registered `logs`, `spans`, and `metrics` tables are backed by `ListingTable` (Hive-partitioned Parquet files on disk). The in-memory Arrow buffer is **not** currently merged into query results — unflushed events (< 2s old) are invisible to queries. This is a known limitation documented in the flush cycle (§4 above).
- **Supported SQL features (confirmed in production):**
  - `SELECT`, `WHERE`, `ORDER BY`, `LIMIT`, `DISTINCT`
  - `GROUP BY` with any column combination including Hive partition columns (`service`, `date`)
  - Aggregate functions: `COUNT(*)`, `COUNT(*) FILTER (WHERE ...)`, `SUM(...)`, `MIN(...)`, `MAX(...)`, `AVG(...)`
  - Arithmetic in SELECT expressions: `CAST(x AS DOUBLE) / CAST(y AS DOUBLE) AS ratio` — used for server-side error-rate computation
  - `FLOOR(CAST(timestamp AS BIGINT) / ...)` for bucket arithmetic on timestamp columns (DataFusion will not divide a `Timestamp(µs)` value directly; the dashboard casts to `BIGINT` micros first)
  - Scalar subqueries (`SELECT ... FROM (SELECT ... GROUP BY ...) sub`)
  - `IN (...)` predicates
  - `UNION ALL` — used by the dashboard's HTTP charts to combine `spans` rows (Go/Rust SDKs) with `logs.attributes` rows (Node.js/Python SDKs) into one aggregate over the combined population
  - `approx_percentile_cont(col, p)` — built-in DataFusion aggregate (no custom UDAF), used by latency percentile chart
  - JSON extraction via `json_get_str(json, 'key')` (from the `datafusion-functions-json` companion crate, registered in `query_engine.rs`): extracts a key from a JSON string. Missing keys / NULL / malformed input return NULL rather than erroring, so aggregates skip them. Used by the dashboard to read HTTP metadata from `logs.attributes` for Node.js/Python SDKs (e.g. `CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE)`). Note the path is a single literal key segment (`'http.latency_ms'`), not JSONPath syntax (`'$.http.latency_ms'`), because the attributes JSON is a flat object whose keys contain dots.
  - String results serialize correctly for both `Utf8` and `Utf8View` (DataFusion 54's `StringView` default for string literals and `CAST(... AS VARCHAR)`), so `CAST(status_code AS VARCHAR)` returns real values, not type names
- **Confirmed not available:**
  - DDL (`CREATE TABLE`, `DROP TABLE`, `ALTER`) — rejected at plan time
  - DML (`INSERT`, `UPDATE`, `DELETE`) — rejected at plan time
  - Exact percentile functions (`PERCENTILE_CONT`, `PERCENTILE_DISC`) — only `approx_percentile_cont` is available
- **Table status:**
  - `logs` — fully implemented (14-column schema, ingested, flushed, queryable). Also carries HTTP request metadata for the Node.js and Python SDKs as `LogEvent.attributes` keys (`http.method`, `http.route`, `http.status_code`, `http.latency_ms`, `logger_name = "greplog.http"`) — the dashboard extracts these via `json_get_str()`.
  - `spans` — fully implemented (13-column schema: `id`, `correlation_id`, `parent_correlation_id`, `service`, `name`, `route`, `method`, `status_code` Int32, `latency_ms` Float64, `is_error` Boolean, `start_time`/`end_time` TimestampMicrosecond, `attributes`; ingested by Rust and Go SDKs plus the benchmark, flushed to Parquet, queryable). The Go/Rust SDKs send HTTP metadata as `Span` messages; Node.js/Python send the same data as log attributes (see SDK wire-format reconciliation in `ROADMAP.md`). The `spans` table has **no `timestamp`, `level`, `message`, or `line` columns** — its time column is `start_time`, and the error signal is `status_code` (the `is_error` flag is derived only from the Span `error` string field, which the HTTP middleware never sets, so it is not a status proxy).
  - `metrics` — registered but empty (no metric ingestion yet)
- **Filter predicate translation for HTTP charts:** the dashboard's three HTTP charts combine both wire shapes with `UNION ALL`, and translate the user's filter per arm so both arms narrow the same population. Key mapping: the Node.js/Python HTTP middleware derives `level` from the response status (`>=500` → `error`, `400-499` → `warn`, `<400` → `info`), so `level` on a `greplog.http` row IS a status bucket; the spans arm therefore maps `level` filters to `status_code` predicates (e.g. `level IN ('error','critical','fatal')` → `status_code >= 500`), `message LIKE` → `name LIKE`/`route LIKE`, and `line` (status chips) → `status_code`. See `useAnalytics.ts` `httpArmPredicates` and the "HTTP metrics: dual source" section in `docs/architecture/dashboard.md`.
- **Row cap:** 10,000 rows per query (applied automatically; a smaller `LIMIT` in user SQL takes precedence).
- **Timeout:** 30 seconds per query.
- **Memory:** 256 MiB pool; DataFusion returns a clean error rather than OOM-killing on overage.
- **Mutation safety:** `SQLOptions` disables DDL, DML, and `COPY` at the logical-plan level before execution.
- **File-list cache:** 10-second TTL (see `LIST_FILES_CACHE_TTL_SECS` in `query_engine.rs` for rationale). New Parquet files become visible within one compaction cycle.

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
