[![License](https://img.shields.io/badge/license-Greplog%20Non--Commercial-blue)](#license)

Greplog — Open-source observability for AI-assisted coders and small dev teams.

`npm i -g greplog && greplog dev` → dashboard in <60s. Zero Docker, zero config.

- New to Greplog? Start with [What is Greplog?](#what-is-greplog) and [Quick start](#quick-start)
- Ready to build from source? Jump to [Build from source](#build-from-source)
- Want to contribute? See [Contributing](#contributing)

## What is Greplog?

Greplog is a local-first observability tool that captures logs, errors, and HTTP metrics from your services — no infrastructure, no agents, no cloud setup. It runs as a single Rust binary on your machine, serving a local React dashboard at `localhost:4317`.

It auto-detects your framework and attaches instrumentation via SDKs for **Node**, **Python**, **Go**, and **Rust**.

### Key use cases

- **Local development debugging:** See all your services' logs and errors in one interleaved timeline during dev.
- **Monorepo observability:** Multiple services in one workspace stream to the same agent — no per-service setup.
- **Incident reproduction:** Run `greplog dev` alongside your `docker-compose up`, reproduce the bug, and inspect every log, error, and HTTP call across every service.
- **Lightweight production for small teams:** Self-hosted, no external dependencies, no telemetry leaving your machine.

## Why Greplog?

|                | Greplog                       | Sentry / Datadog              | ELK / Grafana Loki            |
|----------------|-------------------------------|-------------------------------|-------------------------------|
| Setup time     | <60s (`npm i -g && greplog dev`) | 10-30 min (SDK + cloud onboarding) | Hours (Docker Compose, config) |
| Infrastructure | Zero — runs on your machine   | Requires their cloud or self-hosted infra | Requires Elasticsearch / Loki, Promtail |
| Docker needed  | No                            | No                            | Yes                           |
| Cost           | Free (self-hosted)            | Per-event / per-seat pricing  | Infrastructure + ops          |
| SDK footprint  | Tiny — Protobuf over UDS/TCP  | Heavy — HTTP(S) batching      | Heavy — HTTP(S) shipping      |

Greplog is designed for the **gap between `console.log` and a full observability stack**.

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
import greplog from 'greplog';
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

- **Unified timeline** — All services' logs, errors, and HTTP calls interleaved by timestamp.
- **Service badges** — Color-coded tags per service (`[web]`, `[api]`, `[worker]`).
- **Filter bar** — Query by `service:`, `route:/payments/*`, `status:>=500`, etc.
- **Saved views** — Save filters as named presets (e.g., "Payment errors").
- **Latency & error-rate graphs** — DuckDB-aggregated percentiles over time.
- **Framework auto-detection** — Express, Fastify, NestJS, Next.js, Flask, Django, FastAPI, Gin, Echo, Fiber, Axum, Actix, and more.
- **Docker-friendly** — Falls back to TCP when UDS can't cross the container boundary.
- **PII redaction** — Configurable field scrubbing in SDK and agent.
- **Pseudo-tracing** — Chronological alignment via system clock + optional `correlation_id`.

## SDKs

| Language | Package | Status |
|----------|---------|--------|
| Node/TypeScript | `@greplog/node` | Planned |
| Python | `greplog` (PyPI) | Planned |
| Go | `github.com/greplog/greplog-go` | Planned |
| Rust | `greplog` (crates.io) | Planned |

All SDKs:
- Communicate over **Protobuf** via UDS (host) or TCP (Docker fallback).
- **Fail open** — one warning on startup, then silent drops if the agent isn't running.
- **Never crash** the host app.
- Accept **redaction hooks** for pre-ship scrubbing.

## Architecture overview

```
           ┌─────────────────┐
           │   Node App      │──UDS──┐
           └─────────────────┘       │
           ┌─────────────────┐       │   ┌──────────────────────────┐
           │   Go API        │──UDS──├──▶│     Greplog Agent        │
           └─────────────────┘       │   │  (Rust daemon, local)    │
           ┌─────────────────┐       │   │                          │
           │   Python API    │──TCP──┘   │  UDS/TCP ingest          │
           │   (Docker)      │ fallback  │  DuckDB writer           │
           └─────────────────┘          │  Parquet WAL              │
                                         │  HTTP query API          │
                                         │  Embedded React dashboard│
                                         └──────────┬───────────────┘
                                                    │
                                         ┌──────────▼───────────────┐
                                         │  localhost:4317          │
                                         │  (React Dashboard)       │
                                         └──────────────────────────┘
```

The agent is a single Rust binary per workspace. SDKs never talk to the network — they write to a local `.greplog/greplog.sock`. See [docs/architecture/](docs/architecture/) for the full design.

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
./target/release/greplog-cli dev
```

## Project structure

```
greplog/
├── crates/
│   ├── greplog-core/       # Shared types, Protobuf wire format, redaction
│   ├── greplog-agent/       # Local daemon (ingest, store, query, detect, server)
│   └── greplog-cli/         # CLI (`greplog dev`, `greplog init`, `greplog status`)
├── sdks/
│   ├── node/               # @greplog/node
│   ├── python/             # greplog (PyPI)
│   ├── go/                 # github.com/greplog/greplog-go
│   └── rust/               # greplog crate
├── dashboard/              # React app (embedded in agent binary)
└── docs/                   # Documentation
```

## License

Greplog is released under the [Greplog Non-Commercial License](LICENSE). Self-hosting, personal use, and development use are free. Commercial use and reselling require a commercial license.

See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome. Please open an issue or pull request on GitHub.

This project uses a custom Contributor License Agreement. By contributing, you agree to release your contributions under the terms of the [Greplog Non-Commercial License](LICENSE).

## Community

- [GitHub Issues](https://github.com/greplog/greplog/issues)
- [GitHub Discussions](https://github.com/greplog/greplog/discussions)
