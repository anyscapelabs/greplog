<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/branding/logo/wordmark/wordmark-white.svg">
    <img alt="Greplog" src="assets/branding/logo/wordmark/wordmark-black.svg" height="60">
  </picture>
</p>

<p align="center">
  [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
</p>

<p align="center">Open-source observability for backend services — built for small dev teams and AI-assisted coders.</p>

<p align="center">`npm i -g greplog && greplog dev` → dashboard in <60s. Zero Docker, zero config.</p>

- New to Greplog? Start with [What is Greplog?](#what-is-greplog) and [Quick start](#quick-start)
- Ready to build from source? Jump to [Build from source](#build-from-source)
- Want to contribute? See [Contributing](#contributing)

## What is Greplog?

Greplog is a local-first observability tool for backend services — it captures logs, errors, and HTTP metrics from your APIs and workers, no infrastructure, no agents, no cloud setup. It runs as a single Rust binary on your machine, serving a local React dashboard at `localhost:4317`.

It auto-detects your framework and attaches instrumentation via SDKs for **Node**, **Python**, **Go**, and **Rust**.

### Key use cases

- **Local development debugging:** See all your services' logs and errors in one interleaved timeline during dev.
- **Monorepo observability:** Multiple services in one workspace stream to the same agent — no per-service setup.
- **Incident reproduction:** Run `greplog dev` alongside your `docker-compose up`, reproduce the bug, and inspect every log, error, and HTTP call across every service.
- **Lightweight production for small teams:** Self-hosted, no external dependencies, no telemetry leaving your machine.

Greplog is designed for the **gap between `console.log` and a full observability stack** — for the backend services doing the real work, not the frontend consuming them.

## Quick start

```sh
npm install -g greplog
greplog dev
```

Open `http://localhost:4317` in your browser.

### What just happened

1. `greplog` downloaded the precompiled agent binary for your platform.
2. The agent detected your project's frameworks and service identity.
3. It started a local UDS socket at `.greplog/greplog.sock` and an HTTP server at `:4317`.
4. SDKs in your services auto-connect and stream logs, errors, and HTTP metrics.
5. The dashboard renders a unified timeline of all events.

### Per-language SDK setup

```ts
// Node — auto-instrumenting
import { greplog } from 'greplog';
greplog.init();
```

```python
# Python — auto-instrumenting
import greplog
greplog.init()
```

```go
// Go
import "github.com/greplog/greplog-go"

greplog.Init()
```

```rust
// Rust
greplog::init();
```

## Features

- **Unified timeline** — All services' logs, errors, and HTTP calls interleaved by timestamp. ✅ Shipped
- **Service badges** — Color-coded tags per service (`[web]`, `[api]`, `[worker]`). ✅ Shipped
- **Filter bar** — Query by `service:`, `route:/payments/*`, `status:>=500`, etc. ✅ Shipped
- **Saved views** — Save filters as named presets (e.g., "Payment errors"). 📋 Planned
- **Crash-safe WAL** — Every event is written to a write-ahead log before acknowledgment; zero data loss on crash. ✅ Shipped
- **Content-addressed dedup** — Duplicate events (from SDK retries, network jitter) are eliminated before storage. ✅ Shipped
- **Arrow-native storage** — Events accumulated in an in-memory Arrow buffer, flushed to partitioned Parquet files. ✅ Shipped
- **Auto-compaction** — Background compaction merges small Parquet files into larger partitions; configurable schedule. ✅ Shipped
- **Latency & error-rate graphs** — Query-engine aggregated percentiles over time. 📋 Planned
- **Framework auto-detection** — Express, Fastify, NestJS, Next.js, Flask, Django, FastAPI, Gin, Echo, Fiber, Axum, Actix, and more. ✅ Shipped
- **Docker-friendly** — Falls back to TCP when UDS can't cross the container boundary. ✅ Shipped
- **PII redaction** — Configurable field scrubbing in SDK and agent. ✅ Shipped
- **Pseudo-tracing** — Chronological alignment via system clock + optional `correlation_id`. ✅ Shipped

## SDKs

| Language | Package | Status |
|----------|---------|--------|
| Node/TypeScript | `greplog` (npm) | ✅ Shipped |
| Python | `greplog` (PyPI) | ✅ Shipped |
| Go | `github.com/greplog/greplog-go` | ✅ Shipped |
| Rust | `greplog` (crates.io) | ✅ Shipped |

> Status above reflects source completeness. See each package's registry page for the latest published version.

All SDKs:
- Communicate over **Protobuf** via UDS (host) or TCP (Docker fallback).
- **Fail open** — one warning on startup, then silent drops if the agent isn't running.
- **Never crash** the host app.
- Accept **redaction hooks** for pre-ship scrubbing.

## Architecture overview

```
           ┌─────────────────┐
           │   Node API      │──UDS──┐
           └─────────────────┘       │
           ┌─────────────────┐       │   ┌────────────────────────────────────┐
           │   Go API        │──UDS──├──▶│          Greplog Agent             │
           └─────────────────┘       │   │          (Rust daemon)             │
           ┌─────────────────┐       │   │                                    │
           │   Python Worker │──TCP──┘   │  ┌────────┐  ┌──────────────────┐  │
           │   (Docker)      │ fallback  │  │ Ingest │─▶│ WAL + Dedup      │  │
           └─────────────────┘          │  │ UDS/TCP│  │ (crash-safe)      │  │
                                         │  └────────┘  └────────┬─────────┘  │
                                         │            ┌──────────▼──────────┐ │
                                         │            │ Arrow Buffer        │ │
                                         │            │ (in-memory batch)   │ │
                                         │            └──────────┬──────────┘ │
                                         │            ┌──────────▼──────────┐ │
                                         │            │ Parquet Flush       │ │
                                         │            │ (every 2s, deduped) │ │
                                         │            └──────────┬──────────┘ │
                                         │            ┌──────────▼──────────┐ │
                                         │            │ Compaction          │ │
                                         │            │ (background, hourly)│ │
                                         │            └──────────┬──────────┘ │
                                         │  ┌────────────────────▼─────────┐ │
                                         │  │ HTTP Query API + Dashboard  │ │
                                         │  │ (localhost:4317)             │ │
                                         │  └──────────────────────────────┘ │
                                         └────────────────────────────────────┘
```

The agent is a single Rust binary per workspace. SDKs never talk to the network — they write to a local `.greplog/greplog.sock`. Events pass through a crash-safe WAL, content-addressed dedup, and an Arrow-native buffer before being flushed to partitioned Parquet files with background compaction. See [docs/architecture/](docs/architecture/) for the full design.

## Performance

Greplog is single-node and local-first by design — it's built to handle the log/error/metric volume of local development and small-team production workloads, not to compete on raw ingest throughput with distributed observability backends.

Reproducible throughput benchmarks, methodology, and hardware specs live in [`bench/`](bench/) rather than as headline numbers here — run them yourself against your own hardware if throughput ceilings matter for your use case. Performance work is an active, ongoing area; see [`ROADMAP.md`](ROADMAP.md) for what's planned.

## Build from source

### Prerequisites

- Rust toolchain (1.80+)
- Protobuf compiler (`protoc`)
- Node.js 20+ (for the dashboard build)

### Build

```sh
git clone https://github.com/greplog/greplog
cd greplog

# Build the Rust agent
cargo build --release -p greplog-agent

# Build the dashboard
cd dashboard && npm install && npm run build && cd ..

# Build the CLI
cargo build --release -p greplog-cli

# Start locally
./target/release/greplog dev
```

## Project structure

```
greplog/
├── crates/
│   ├── greplog-core/        # Shared types, Protobuf wire format, redaction
│   └── greplog-agent/       # Local daemon (ingest, store, query, detect, server)
│       ├── ingest/          # UDS & TCP listener, batching channels
│       ├── store/           # WAL, dedup, Arrow buffer, Parquet flush, compaction
│       ├── query_engine.rs  # Parquet/Arrow query layer served to dashboard
│       ├── detect/          # framework auto-detection engine
│       └── server/          # embedded HTTP server (axum) + static dashboard
├── cli/
│   └── greplog-cli/         # CLI (`greplog dev`, `greplog init`, `greplog status`)
├── sdks/
│   ├── node/               # greplog (npm)
│   ├── python/             # greplog (PyPI)
│   ├── go/                 # github.com/greplog/greplog-go
│   └── rust/               # greplog crate
├── bench/                  # Reproducible throughput/latency benchmarks
├── dashboard/              # React app (embedded in agent binary)
├── docs/                   # Documentation
└── assets/                 # Branding and design assets
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

## Contributing

Contributions are welcome. Please see [CONTRIBUTING.md](CONTRIBUTING.md) for build/test setup and PR conventions, or just open an issue or pull request on GitHub.

By contributing, you agree to license your contributions under the Apache 2.0 License.

## Security

If you've found a security vulnerability, please see [SECURITY.md](SECURITY.md) for responsible disclosure rather than opening a public issue.

## Community

- [GitHub Issues](https://github.com/greplog/greplog/issues)
- [GitHub Discussions](https://github.com/greplog/greplog/discussions)
