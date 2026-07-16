# Greplog v0.1 — Implementation Guide

This document covers only the **CLI, Agent, and Core** crates. SDKs (Node, Python, Go, Rust) are excluded.

---

## Table of Contents

1. [System Architecture](#1-system-architecture)
2. [Crate: greplog-core](#2-crate-greplog-core)
3. [Crate: greplog-agent](#3-crate-greplog-agent)
4. [Crate: greplog-cli](#4-crate-greplog-cli)
5. [What's Left to Implement](#5-whats-left-to-implement)

---

## 1. System Architecture

```
Node App ──UDS──┐
Go API  ──UDS──┤
Python  ──TCP──┘  (Docker fallback)
                  │
                  ▼
          ┌──────────────────┐
          │  greplog-agent   │
          │  (Rust daemon)   │
          │                  │
          │  ┌─ ingest ────┐ │
          │  │  TCP + UDS  │ │
          │  └──────┬──────┘ │
          │  ┌──────▼──────┐ │
          │  │  store      │ │
          │  │  DuckDB     │ │
          │  └──────┬──────┘ │
          │  ┌──────▼──────┐ │
          │  │  query      │ │
          │  │  SQL engine │ │
          │  └──────┬──────┘ │
          │  ┌──────▼──────┐ │
          │  │  server     │ │
          │  │  HTTP API   │ │
          │  └─────────────┘ │
          └────────┬─────────┘
                   │
          ┌────────▼─────────┐
          │  React Dashboard │
          │  localhost:4317  │
          └──────────────────┘
```

### Data Flow

1. **SDK/App** sends `IngestBatch` (Protobuf) over UDS or TCP
2. **ingest** decodes length-delimited frames, forwards raw bytes to channel
3. **store** accumulates batches, flushes to DuckDB every 2s
4. **query** provides read-only SQL access to stored data
5. **server** exposes HTTP endpoints consumed by the CLI and dashboard

---

## 2. Crate: greplog-core

**Purpose:** Shared data model, wire format, and utilities. Zero runtime dependencies beyond Prost.

### 2.1 Protobuf Schema (`proto/greplog/v1/events.proto`)

Compiled at build time via `prost-build` into native Rust structs.

#### Messages

| Message | Fields | Purpose |
|---|---|---|
| `LogEvent` | `service_name`, `message`, `level`, `timestamp_ns`, `logger_name`, `file`, `line`, `correlation_id`, `attributes` (map), `stack_trace`, `exception_type`, `exception_message` | Structured log entry with optional exception info |
| `Span` | `service_name`, `name`, `start_time_ns`, `end_time_ns`, `correlation_id`, `parent_correlation_id`, `route`, `method`, `status_code`, `error`, `attributes` (map), `kind` | Distributed tracing span with HTTP/RPC metadata |
| `Metric` | `service_name`, `name`, `value`, `timestamp_ns`, `labels` (map), `type`, `bucket_values` | Counter, gauge, or histogram datapoint |
| `IngestBatch` | `service_name`, `instance_id`, `batch_seq`, `logs`, `spans`, `metrics` | Wire envelope for transport |
| `IngestResponse` | `accepted`, `events_count`, `error` | Acknowledgement **(defined but not yet sent by agent)** |

#### Enums

| Enum | Variants |
|---|---|
| `SpanKind` | `Unspecified`, `Internal`, `Server`, `Client`, `Producer`, `Consumer` |
| `MetricType` | `Unspecified`, `Counter`, `Gauge`, `Histogram` |

### 2.2 Redaction Module (`src/redact.rs`)

Three strategies for scrubbing sensitive data before storage:

| Mode | Behavior |
|---|---|
| `Full` | Replaces entire value with `[REDACTED]` |
| `Partial` | Keeps first 2 + last 2 chars, masks middle with `***`; strings ≤4 chars become `[***]` |
| `Hash` | One-way `DefaultHasher`, formatted as `[HASH:{hex}]` |

Applied automatically at write time for attribute keys containing `password`, `token`, `secret` (Full) or `email` (Partial).

### 2.3 Schema Constants (`src/schema.rs`)

**Well-known attribute keys** (`schema::keys`): `service.name`, `deployment.environment`, `host.hostname`, HTTP metadata (`http.method`, `http.route`, `http.status_code`, `http.request.body`, `http.response.body`, `http.latency_ms`), DB metadata (`db.system`, `db.statement`, `db.operation`), exception fields.

**Log level constants** (`schema::levels`): `trace`, `debug`, `info`, `warn`, `error`, `fatal`.

**Helper methods** on generated types:
- `LogEvent::is_error()` — `level in ("error", "fatal")`
- `Span::duration_ns()` / `Span::duration_ms()` — computed from `end_time_ns - start_time_ns`
- `Span::is_http()` — `method` or `route` is non-empty
- `Metric::is_counter()` / `is_gauge()` / `is_histogram()` — checks `MetricType`

### 2.4 Implementation Logic

**Why Protobuf over JSON?** Strict schema evolution, smaller wire footprint, native support across all target SDK languages (Go, Python, Node, Rust). The wire format is locked per ADR.

**Why `prost` instead of `tonic`?** The agent doesn't use gRPC — it uses raw length-delimited framing over TCP/UDS. Prost gives us just the serialization without the gRPC stack.

---

## 3. Crate: greplog-agent

**Purpose:** The local observability daemon. Single Rust binary handles ingestion, storage, query, and HTTP serving.

### 3.1 Entry Point (`lib.rs`)

**`Config`** — CLI-driven configuration (via `clap::Parser`):

| Field | Default | Description |
|---|---|---|
| `workspace` | `.` | Project root |
| `tcp_port` | 4318 | TCP ingest port |
| `ui_port` | 4317 | HTTP API port |
| `flush_interval_secs` | 2 | DuckDB flush interval |
| `socket_path` | `.greplog/greplog.sock` | UDS path |

**`run(config)`** — Main agent loop:

1. Creates `.greplog/` directory
2. Sets up MPSC channel (capacity 1024)
3. Initializes DuckDB writer, spawns flush loop
4. Starts TCP + UDS ingest listeners
5. Creates read-only query engine (in-memory DuckDB attached to store.db)
6. Starts Axum HTTP server
7. `tokio::select!` waits for any subsystem to exit or SIGINT
8. Cleans up socket file on shutdown

### 3.2 Ingest Server (`ingest/mod.rs`)

**Transport protocol:** Length-delimited framing — `[4-byte LE u32 length][Protobuf bytes]`, max 16 MB per frame.

**Dual transport:**
- **TCP** on `127.0.0.1:4318` — for Dockerized apps that can't access the host UDS
- **Unix Domain Socket** at `<workspace>/.greplog/greplog.sock` — for host-native apps, zero network config

**Per-connection handler:** Wraps the stream in `tokio_util::codec::FramedRead` with a custom `LengthDelimitedCodec`, forwards decoded `Bytes` into the shared MPSC channel.

**Notable omission:** No `IngestResponse` is sent back — the client has no way to know if its batch was accepted or rejected.

### 3.3 Store / Writer (`store/mod.rs`)

**Schema** (auto-created on first run):

```sql
CREATE TABLE IF NOT EXISTS logs (
    id VARCHAR PRIMARY KEY,
    service VARCHAR NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    level VARCHAR,
    message VARCHAR,
    attributes JSON
);

CREATE TABLE IF NOT EXISTS spans (
    id VARCHAR PRIMARY KEY,
    correlation_id VARCHAR,
    service VARCHAR NOT NULL,
    name VARCHAR,
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP NOT NULL,
    attributes JSON
);

CREATE TABLE IF NOT EXISTS metrics (
    id VARCHAR PRIMARY KEY,
    service VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    value DOUBLE NOT NULL,
    labels JSON
);
```

**Write path:**
1. Background loop accumulates `IngestBatch` messages from the channel
2. Every `flush_interval_secs` (default 2s), spawns a blocking task
3. Opens a DuckDB transaction, iterates all buffered batches
4. Each row gets a **UUID v4** primary key
5. Nanosecond timestamps converted via `to_timestamp(? / 1000000000.0)`
6. Attributes/labels serialized to JSON
7. `INSERT OR IGNORE` to skip duplicates
8. In-flight redaction of sensitive fields before insert

**Redaction on write:** Before inserting, attribute maps are scanned — keys matching `password`, `token`, `secret` get `RedactionMode::Full`; keys matching `email` get `RedactionMode::Partial`.

### 3.4 Query Engine (`query/mod.rs`)

**Setup:** Creates an **in-memory** DuckDB connection, then `ATTACH`s the on-disk `store.db` as `READ_ONLY` with alias `store`. This keeps write traffic isolated from reads.

**Query safety validator** — gates every SQL query:

- **Rejects** multi-statement input (anything after a semicolon that isn't whitespace)
- **Blocks** mutation statements: `INSERT`, `UPDATE`, `DELETE`, `DROP`, `ALTER`, `CREATE`, `TRUNCATE`, `REPLACE`, `MERGE`, `GRANT`, `REVOKE`, `ATTACH`, `DETACH`, `COPY`, `LOAD`, `INSTALL`, `PRAGMA`, `SET`, `BEGIN`, `COMMIT`, `ROLLBACK`, `CHECKPOINT`
- **Allows only:** `SELECT`, `EXPLAIN`, `DESCRIBE`, `SHOW`, `WITH` (CTEs)
- Trailing semicolons are permitted

**Result serialization:** Query results are converted from Arrow `RecordBatch` to `Vec<Vec<serde_json::Value>>` with type-aware mapping (booleans, integers, floats, strings, timestamps).

### 3.5 HTTP Server (`server/mod.rs`)

**Framework:** Axum 0.7 with CORS (any origin, GET/POST).

| Endpoint | Method | Handler | Description |
|---|---|---|---|
| `/health` | GET | Returns `{ status: "ok", version }` | Liveness probe |
| `/status` | GET | Returns `{ status: "running", version }` | Redundant with `/health` |
| `/query` | POST | Accepts `{ sql: "..." }`, returns `{ columns, rows, row_count }` | SQL query interface |
| `/detect` | GET | Returns `[{ service_name, language, framework, project_file }]` | Workspace framework detection |
| `/resources` | GET | Returns `{ cpu, memory, disk, load_avg, uptime_secs }` | System resource snapshot |

### 3.6 Framework Detection (`detect/mod.rs`)

Scans workspace for project files and identifies frameworks:

| File | Languages | Frameworks |
|---|---|---|
| `package.json` | Node.js | Next.js, Express |
| `Cargo.toml` | Rust | Axum, Actix Web |
| `go.mod` | Go | Gin, Echo, Fiber, Chi, Gorilla Mux |

Service names are extracted from the project file's `name` field.

### 3.7 System Resources (`detect/system.rs`)

Uses `sysinfo` crate to collect:
- **Memory:** total, used, usage %
- **CPU:** core count, global usage %, brand string
- **Disk:** workspace mount point usage
- **Load average:** 1, 5, 15 minute averages
- **Uptime:** system uptime in seconds

---

## 4. Crate: greplog-cli

**Purpose:** Thin CLI wrapper over the agent. Two commands: `dev` and `status`.

### 4.1 Command: `greplog dev`

**Options:** `--foreground`, `--port` (default 4317), `--tcp-port` (default 4318), `--workspace`

**Foreground mode:**
1. Prints connection info (dashboard URL, workspace, socket path, TCP address)
2. Calls `greplog_agent::run(config).await` directly
3. Blocks until agent exits

**Background mode (default):**
1. Spawns agent in a `tokio::spawn` task
2. Polls `GET /health` every 200ms for up to 5 seconds
3. If healthy: prints ready banner + "running in background" message, then blocks on the task handle
4. If unhealthy after 5s: prints error, aborts task, exits with code 1

### 4.2 Command: `greplog status`

**Options:** `--port` (default 4317)

Sequentially calls the running agent's HTTP API:

1. **`GET /health`** — reports if agent is running
2. **`GET /resources`** — prints CPU cores/usage and memory usage
3. **`GET /detect`** — lists detected services
4. **`POST /query`** — runs `SELECT count(*) FROM logs/spans/metrics` to show event counts

### 4.3 Banner (`commands/mod.rs`)

Uses `figlet-rs` with the `standard` font to render "Greplog" in bright cyan, followed by the version number in dimmed text.

---

## 5. What's Left to Implement

### 5.1 greplog-agent

| Feature | Priority | Description |
|---|---|---|
| **Ingest acknowledgment** | High | Send `IngestResponse` protobuf back to clients so they know their batch was accepted/rejected |
| **Dashboard embedding** | High | Bundle the built React dashboard into the agent binary (e.g., `rust-embed`) and serve it from the HTTP server instead of requiring a separate dev server |
| **Database indexes** | High | Add indexes on `service`, `timestamp`, `level` for the most common query patterns |
| **Parquet WAL / compaction** | Medium | Periodically compact old DuckDB data into Parquet files partitioned by hour (described in architecture docs, not implemented) |
| **Metric aggregation & rollups** | Medium | Pre-compute time-bucketed aggregates for dashboard latency/error-rate charts |
| **Span trace tree queries** | Medium | Add query support for reconstructing trace waterfalls from `correlation_id` and `parent_correlation_id` |
| **Retention policies** | Medium | Configurable data retention (e.g., "keep 7 days, then archive or delete") |
| **More language detection** | Medium | Add Python (Flask, Django, FastAPI), Java (Spring Boot), Ruby (Rails) |
| **Backpressure / rate limiting** | Medium | Drop or throttle clients when the MPSC channel is full instead of unbounded blocking |
| **TLS support** | Low | Optional TLS for TCP ingest and HTTP API |
| **Auth / API keys** | Low | Simple token-based auth on HTTP endpoints |
| **Concurrent query connections** | Low | Connection pool instead of single `Arc<Mutex<Connection>>` |
| **Integration tests** | Medium | End-to-end tests: start agent, send data via TCP/UDS, query via HTTP |

### 5.2 greplog-cli

| Feature | Priority | Description |
|---|---|---|
| **Serve dashboard** | High | When running `greplog dev`, the agent should serve the dashboard HTML/JS/CSS so the user can open `localhost:4317` in a browser |
| **Port conflict detection** | Low | Check if ports 4317/4318 are already in use before starting |
| **Graceful shutdown timeout** | Low | Add a timeout to force-kill the agent if it doesn't shut down within N seconds |

### 5.3 greplog-core

| Feature | Priority | Description |
|---|---|---|
| **No major gaps** | — | The core data model and utilities are complete for v0.1. Future work: schema evolution, new metric types |

### 5.4 Dashboard (for context — not SDK)

| Feature | Priority | Description |
|---|---|---|
| **Connect to real agent API** | High | Replace all mock data with actual `fetch()` calls to the agent's `/query`, `/resources`, `/detect` endpoints |
| **Errors page content** | Medium | Implement error grouping, stack trace display |
| **Traces page** | Medium | Trace waterfall visualization from span data |
| **Views page** | Medium | Saved view CRUD (save/load filter configurations) |
| **Services page** | Medium | Per-service overview with health status |
| **Patterns page** | Low | Log pattern detection and grouping |
| **Live tail** | Low | WebSocket or polling-based live log stream |
