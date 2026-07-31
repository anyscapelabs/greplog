# System Overview

> **Status:** ✅ Shipped (v0.1)

Open-source observability for backend services — built for small dev teams and AI-assisted coders.
Core promise: `npm i -g greplog && greplog dev` → dashboard in <60s, zero Docker, zero config.

## High-Level System Map

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                             DEVELOPER MACHINE                                │
│                                                                              │
│  ┌───────────────┐         ┌──────────────────────────────────────────────┐  │
│  │ Node API      │──UDS───▶│              Greplog Agent                   │  │
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

Events flow through four stages inside the agent (WAL → dedup → Arrow buffer → Parquet flush/compaction). See [`agent-pipeline.md`](agent-pipeline.md) for the full canonical description.

## Monorepo Structure

```
greplog/
├── crates/
│   ├── greplog-core/                 # shared types: LogEvent, Span, Metric schemas
│   │   ├── src/
│   │   ├── tests/
│   │   └── Cargo.toml
│   │
│   └── greplog-agent/                # the local daemon (`greplog dev`)
│       ├── src/
│       │   ├── lib.rs
│       │   ├── main.rs
│       │   ├── ingest/               # UDS & TCP listener, batch framing
│       │   ├── store/                # WAL, dedup, Arrow buffer, flush, compaction
│       │   ├── query_engine.rs
│       │   ├── detect/
│       │   └── server/
│       ├── tests/
│       └── Cargo.toml
│
├── cli/
│   └── greplog-cli/                  # thin CLI wrapping the agent
│       ├── src/
│       │   ├── main.rs               # `greplog dev`, `greplog init`, `greplog status`
│       │   └── commands/
│       ├── src/bin/
│       │   └── bench_throughput.rs
│       └── Cargo.toml
│
├── sdks/
│   ├── node/
│   ├── python/
│   ├── go/
│   │   ├── src/
│   │   ├── tests/
│   │   └── go.mod
│   └── rust/
│       ├── src/
│       ├── tests/
│       └── Cargo.toml
│
├── bench/
│   ├── ingest_throughput/
│   ├── query_latency/
│   └── README.md
│
├── dashboard/                        # React app (Vite, separate from agent)
│   ├── src/
│   ├── dist/
│   └── package.json
│
├── docs/
│   ├── quickstart.md
│   ├── ROADMAP.md
│   ├── architecture/                 # system docs
│   ├── adr/                          # architecture decision records
│   ├── sdk/                          # SDK design + per-language refs
│   └── distribution/                 # binary distribution docs
│
└── Cargo.toml                        # Rust workspace
```

Why this shape: Rust crates in one Cargo workspace share `greplog-core` (schemas, redaction, Protobuf wire format) so the agent never drifts. SDKs stay separate per-language package repos because each has its own publishing pipeline, but they all speak the same wire protocol defined once in `greplog-core`.
