# System Overview

Open-source observability for AI-assisted coders and small dev teams.
Core promise: `npm i -g greplog && greplog dev` → dashboard in <60s, zero Docker, zero config.

## High-Level System Map

```
┌────────────────────────────────────────────────────────────────────────────┐
│                             DEVELOPER MACHINE                              │
│                                                                            │
│                            ┌─────────────────┐                             │
│  ┌───────────────┐         │ Greplog Agent   │                             │
│  │ Node App (UI) │──UDS───▶│ (Workspace       │                             │
│  └───────────────┘         │  daemon, Rust)   │                             │
│                            │                  │                             │
│  ┌───────────────┐         │ - Channel/Queue  │                             │
│  │ Go API (Host) │──UDS───▶│ - Single-thread  │                             │
│  └───────────────┘         │   DuckDB writer  │                             │
│                            │ - Parquet WAL    │                             │
│  ┌───────────────┐         │ - HTTP :4317     │                             │
│  │ Python API    │──TCP───▶│                  │                             │
│  │ (in Docker)   │fallback │                  │                             │
│  └───────────────┘         └────────┬─────────┘                             │
│                                      │                                      │
│                              ┌───────▼────────────────┐                     │
│                              │ React Dashboard        │                     │
│                              │ (served by agent)      │                     │
│                              │ localhost:4317         │                     │
│                              └────────────────────────┘                     │
└────────────────────────────────────────────────────────────────────────────┘
```

The key architectural decision: the SDK never talks to the network directly. It always writes to the local agent over a workspace-scoped Unix domain socket (falling back to a local TCP port for Dockerized apps). The agent decides whether to persist locally only. This keeps the SDK dead simple (~small dependency footprint) and lets you evolve ingestion, storage, and shipping logic in one Rust codebase instead of reimplementing it four times across SDKs.

## Monorepo Structure

```
greplog/
├── crates/                          # Rust workspace (agent, core)
│   ├── greplog-core/                 # shared types: LogEvent, Span, Metric schemas
│   │   ├── src/
│   │   │   ├── schema.rs             # Arrow schema defs, shared by agent
│   │   │   ├── ingest_proto.rs       # protobuf wire format (locked in)
│   │   │   └── redact.rs             # PII scrubbing rules (shared)
│   │   └── Cargo.toml
│   │
│   ├── greplog-agent/                # the local daemon (`greplog dev`)
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── ingest/                # UDS & TCP listener, batching channels
│   │   │   ├── store/                 # Single-threaded DuckDB writer, Parquet rotation
│   │   │   ├── query/                 # SQL query layer served to dashboard
│   │   │   ├── detect/                # framework auto-detection engine
│   │   │   └── server/                # embedded HTTP server (axum) + static dashboard
│   │   └── Cargo.toml
│   │
│   ├── greplog-cli/                  # thin CLI wrapping the agent
│   │   ├── src/
│   │   │   ├── main.rs                # `greplog dev`, `greplog init`, `greplog status`
│   │   │   └── commands/
│   │   └── Cargo.toml
│   │
│   └── Cargo.toml                     # workspace root
│
├── sdks/
│   ├── node/                         # @greplog/node
│   │   ├── src/
│   │   │   ├── index.ts               # `import greplog from 'greplog'`
│   │   │   ├── init.ts                # greplog.init() — starts detection + hooks
│   │   │   ├── detectors/             # service identity, express, fastify, nest, next
│   │   │   ├── transport/             # UDS client → TCP fallback → agent
│   │   │   └── instrumentation/       # auto-patch http, uncaughtException, etc.
│   │   ├── package.json
│   │   └── tsconfig.json
│   │
│   ├── python/                       # greplog (PyPI)
│   │   ├── greplog/
│   │   │   ├── __init__.py
│   │   │   ├── init.py
│   │   │   ├── detectors/             # service identity, flask, django, fastapi
│   │   │   ├── transport.py
│   │   │   └── instrumentation.py     # sys.excepthook, logging.Handler
│   │   └── pyproject.toml
│   │
│   ├── go/                           # github.com/greplog/greplog-go
│   │   ├── greplog.go                 # greplog.Init()
│   │   ├── detect/                    # module name, gin, echo, fiber, net/http
│   │   ├── transport/
│   │   └── go.mod
│   │
│   └── rust/                         # greplog crate (also reused by agent internally)
│       ├── src/
│       │   ├── lib.rs                 # greplog::init()
│       │   ├── detect/                # cargo name, axum, actix, tower detection
│       │   └── transport/
│       └── Cargo.toml
│
├── dashboard/                        # React app, embedded into agent binary
│   ├── src/
│   │   ├── views/
│   │   │   ├── LogsExplorer/          # unified timeline + filter builder
│   │   │   ├── TraceView/             # pseudo-trace visualization
│   │   │   ├── SavedViews/            # "Payment errors", "Auth errors" presets
│   │   │   └── Dashboards/            # latency graphs, error-rate graphs
│   │   ├── components/
│   │   │   ├── FilterBar/             # query-builder UI (field:op:value chips)
│   │   │   ├── LogTable/              # features service-name badges
│   │   │   └── charts/
│   │   ├── api/                       # talks to agent's local HTTP/SQL query API
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
