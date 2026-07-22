# System Overview

Open-source observability for AI-assisted coders and small dev teams.
Core promise: `npm i -g greplog && greplog dev` → dashboard in <60s, zero Docker, zero config.

## High-Level System Map

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                             DEVELOPER MACHINE                                │
│                                                                              │
│  ┌───────────────┐         ┌──────────────────────────────────────────────┐  │
│  │ Node App (UI) │──UDS───▶│              Greplog Agent                   │  │
│  └───────────────┘         │              (Rust daemon)                   │  │
│                            │                                              │  │
│  ┌───────────────┐         │  ┌─────────┐  ┌──────────────────────────┐  │  │
│  │ Go API (Host) │──UDS───▶│  │ Ingest  │  │ WAL + Dedup             │  │  │
│  └───────────────┘         │  │UDS/TCP  │─▶│ (Write-ahead log,       │  │  │
│                            │  └─────────┘  │  content-addressed idx) │  │  │
│  ┌───────────────┐         │               └───────────┬──────────────┘  │  │
│  │ Python API    │──TCP───▶│               ┌───────────▼──────────────┐  │  │
│  │ (in Docker)   │fallback │               │ Arrow Buffer             │  │  │
│  └───────────────┘         │               │ (single-threaded,       │  │  │
│                            │               │  accumulates batches)   │  │  │
│                            │               └───────────┬──────────────┘  │  │
│                            │               ┌───────────▼──────────────┐  │  │
│                            │               │ Parquet Flush (every 2s) │  │  │
│                            │               │ + WAL truncation         │  │  │
│                            │               └───────────┬──────────────┘  │  │
│                            │               ┌───────────▼──────────────┐  │  │
│                            │               │ Compaction Scheduler     │  │  │
│                            │               │ (background, reads from │  │  │
│                            │               │  query engine, merges   │  │  │
│                            │               │  small partitions)      │  │  │
│                            │               └───────────┬──────────────┘  │  │
│                            │               ┌───────────▼──────────────┐  │  │
│                            │               │ HTTP Query API + Server  │  │  │
│                            │               │ (axum, localhost:4317)   │  │  │
│                            │               └──────────────────────────┘  │  │
│                            └──────────────────────────────────────────────┘  │
│                                                                              │
│                              ┌─────────────────────────────┐                 │
│                              │ React Dashboard             │                 │
│                              │ (embedded, served by agent) │                 │
│                              │ localhost:4317              │                 │
│                              └─────────────────────────────┘                 │
└──────────────────────────────────────────────────────────────────────────────┘
```

The key architectural decision: the SDK never talks to the network directly. It always writes to the local agent over a workspace-scoped Unix domain socket (falling back to a local TCP port for Dockerized apps). The agent decides whether to persist locally only. This keeps the SDK dead simple (~small dependency footprint) and lets you evolve ingestion, storage, and shipping logic in one Rust codebase instead of reimplementing it four times across SDKs.

## Storage Architecture

Events flow through four stages inside the agent:

1. **WAL (Write-Ahead Log)** — Every incoming batch is written to `wal/` as framed Protobuf segments before acknowledgment. On crash recovery, the WAL is replayed to guarantee zero data loss. Each WAL entry carries a content-addressed hash for dedup.

2. **Dedup** — Content-addressed dedup using BLAKE3 hashes of serialized events. Duplicates (from SDK retries, network jitter) are detected before entering the Arrow buffer. The dedup index is maintained in memory and flushed to a persistent index file (`dedup.idx`).

3. **Arrow Buffer** — A single-threaded writer accumulates deduplicated events as Arrow record batches in memory. The buffer stores events as typed Arrow arrays (timestamp, service, level, message, etc.), enabling zero-copy queries and efficient Parquet encoding.

4. **Parquet Flush + Compaction** — Every 2 seconds the buffer flushes to a new Parquet file partitioned by hour (`logs/<date>/<hour>/<seq>.parquet`). A background compaction scheduler periodically merges small files within each partition to improve query performance and reclaim space.

## Monorepo Structure

```
greplog/
├── crates/                          # Rust workspace (agent, core, cli)
│   ├── greplog-core/                 # shared types: LogEvent, Span, Metric schemas
│   │   ├── src/
│   │   │   ├── schema.rs             # Arrow schema defs, shared by agent
│   │   │   ├── gen/                  # protobuf-generated code (prost)
│   │   │   └── redact.rs             # PII scrubbing rules (shared)
│   │   └── Cargo.toml
│   │
│   ├── greplog-agent/                # the local daemon (`greplog dev`)
│   │   ├── src/
│   │   │   ├── lib.rs                # AgentBuilder, agent loop
│   │   │   ├── main.rs               # binary entrypoint
│   │   │   ├── ingest/               # UDS & TCP listener, batch framing
│   │   │   ├── store/                # WAL, dedup, Arrow buffer, Parquet flush, compaction
│   │   │   │   ├── mod.rs            # Store: orchestrates buffer/flush/compaction
│   │   │   │   ├── buffer.rs         # Arrow record-batch accumulator
│   │   │   │   ├── wal.rs            # Write-ahead log + crash recovery
│   │   │   │   ├── dedup.rs          # Content-addressed dedup (BLAKE3)
│   │   │   │   ├── flush.rs          # Parquet writer, partition rotation
│   │   │   │   ├── compaction.rs     # Partition merging (size-based)
│   │   │   │   ├── compaction_scheduler.rs  # Background compaction task
│   │   │   │   ├── schema.rs         # Store-specific Arrow schemas
│   │   │   │   └── io.rs             # File I/O helpers
│   │   │   ├── query_engine.rs       # Parquet/Arrow query layer for dashboard
│   │   │   ├── detect/               # framework auto-detection engine
│   │   │   └── server/               # embedded HTTP server (axum) + static dashboard
│   │   ├── benches/                  # (planned) criterion benchmarks
│   │   └── Cargo.toml
│   │
│   ├── greplog-cli/                  # thin CLI wrapping the agent
│   │   ├── src/
│   │   │   ├── main.rs               # `greplog dev`, `greplog init`, `greplog status`
│   │   │   └── commands/
│   │   ├── src/bin/
│   │   │   └── bench_throughput.rs   # End-to-end throughput benchmark
│   │   └── Cargo.toml
│   │
│   └── Cargo.toml                    # workspace root
│
├── sdks/
│   ├── node/                         # @greplog/node
│   │   ├── src/
│   │   │   ├── index.ts              # `import greplog from 'greplog'`
│   │   │   ├── init.ts               # greplog.init() — starts detection + hooks
│   │   │   ├── detectors/            # service identity, express, fastify, nest, next
│   │   │   ├── transport/            # UDS client → TCP fallback → agent
│   │   │   └── instrumentation/      # auto-patch http, uncaughtException, etc.
│   │   ├── package.json
│   │   └── tsconfig.json
│   │
│   ├── python/                       # greplog (PyPI)
│   │   ├── greplog/
│   │   │   ├── __init__.py
│   │   │   ├── init.py
│   │   │   ├── detectors/            # service identity, flask, django, fastapi
│   │   │   ├── transport.py
│   │   │   └── instrumentation.py    # sys.excepthook, logging.Handler
│   │   └── pyproject.toml
│   │
│   ├── go/                           # github.com/greplog/greplog-go
│   │   ├── greplog.go                # greplog.Init()
│   │   ├── detect/                   # module name, gin, echo, fiber, net/http
│   │   ├── transport/
│   │   └── go.mod
│   │
│   └── rust/                         # greplog crate (also reused by agent internally)
│       ├── src/
│       │   ├── lib.rs                # greplog::init()
│       │   ├── detect/               # cargo name, axum, actix, tower detection
│       │   └── transport/
│       └── Cargo.toml
│
├── dashboard/                        # React app, embedded into agent binary
│   ├── src/
│   │   ├── views/
│   │   │   ├── LogsExplorer/         # unified timeline + filter builder
│   │   │   ├── TraceView/            # pseudo-trace visualization
│   │   │   ├── SavedViews/           # "Payment errors", "Auth errors" presets
│   │   │   └── Dashboards/           # latency graphs, error-rate graphs
│   │   ├── components/
│   │   │   ├── FilterBar/            # query-builder UI (field:op:value chips)
│   │   │   ├── LogTable/             # features service-name badges
│   │   │   └── charts/
│   │   ├── api/                      # talks to agent's local HTTP/SQL query API
│   │   └── main.tsx
│   ├── vite.config.ts
│   └── package.json
│
├── docs/
│   ├── quickstart.md
│   ├── sdk-node.md / sdk-python.md / sdk-go.md / sdk-rust.md
│   └── architecture/                 # this doc
│
├── scripts/
│   ├── build-binaries.sh             # cross-compile agent per OS/arch
│   └── release.sh
│
├── .github/workflows/
│   ├── ci.yml
│   └── release-binaries.yml          # builds + publishes precompiled agent binaries
│
├── Cargo.toml                        # Rust workspace
├── pnpm-workspace.yaml               # JS workspace (node sdk + dashboard)
└── README.md
```

Why this shape: Rust crates in one Cargo workspace share `greplog-core` (schemas, redaction, Protobuf wire format) so the agent never drifts. SDKs stay separate per-language package repos because each has its own publishing pipeline, but they all speak the same wire protocol defined once in `greplog-core`.
