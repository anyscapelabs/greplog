# Roadmap

What's built, what's in progress, and what's planned for Greplog.

## Legend

- ✅ **Shipped (v0.1)** — complete and tested
- 🚧 **In progress** — actively being worked on
- 📋 **Planned** — designed but not started

## Agent

| Feature | Priority | Status |
|---------|----------|--------|
| Ingest acknowledgment (`IngestResponse`) | High | ✅ Done |
| Crash-safe WAL | High | ✅ Done |
| Content-based dedup (BLAKE3) | High | ✅ Done |
| Parquet flush + compaction | High | ✅ Done |
| Query engine (in-memory buffer + Parquet reads) | High | ✅ Done |
| HTTP query API | High | ✅ Done |
| UDS + TCP ingest | High | ✅ Done |
| Framework workspace detection (/detect) | High | ✅ Done |
| System resources endpoint (/resources) | High | ✅ Done |
| Metric aggregation & rollups | Medium | 📋 Pending |
| Span trace tree queries | Medium | 📋 Pending |
| Retention policies | Medium | 📋 Pending |
| More language detection | Medium | 📋 Pending |
| Compression options (Zstd/snappy) | Medium | 📋 Pending |
| TLS support | Low | 📋 Pending |
| Auth / API keys | Low | 📋 Pending |

## CLI

| Feature | Priority | Status |
|---------|----------|--------|
| `greplog dev` (start agent) | High | ✅ Done |
| `greplog status` (health check) | High | ✅ Done |
| `greplog init` (framework detection) | High | ✅ Done |
| Port conflict detection | Low | 📋 Pending |
| Graceful shutdown timeout | Low | 📋 Pending |

## Core

| Feature | Priority | Status |
|---------|----------|--------|
| Protobuf schema + generated code | High | ✅ Done |
| Arrow schema definitions | High | ✅ Done |
| PII redaction | High | ✅ Done |
| ULID generation | High | ✅ Done |

## Dashboard

| Feature | Priority | Status |
|---------|----------|--------|
| Global filter bar (URL-synced FilterState) | High | ✅ Done |
| Connect to real agent API | High | ✅ Done |
| Live/refresh/auto-refresh unified mechanism | High | ✅ Done |
| No fabricated chart data (honest empty states) | High | ✅ Done |
| Analytics real data: ingestion, error rate, service health, noisy services, severity | High | 🚧 In progress (queries wired, chart stubs ready) |
| Charts: latency percentiles, status codes, avg response time (require `spans` table) | Medium | 📋 Pending — needs `FROM spans` query support |
| Charts: system metrics, service requests/latency by service | Low | 📋 Pending — needs new data source or spans integration |
| Errors page (wired filtering) | Medium | ✅ Done |
| Services page (sidebar filtering) | Medium | ✅ Done |
| Service Cards: sparklines from time-bucketed queries | Medium | ✅ Done |
| ServicesDrawer: Recent Errors from `useErrors` + Related Logs from `useLogs` | Medium | ✅ Done |
| Service Details: honest "Streaming since" proxy from `MIN(timestamp)` | Medium | ✅ Done |
| Sidebar filter real counts from query | Medium | 📋 Pending — plug filter-section aggregation queries |
| Service version/hostname in Service Details | Low | 📋 Pending — requires cross-SDK protocol change (new handshake field) |
| Traces page | Medium | 📋 Pending |
| Views page (saved filters) | Medium | 📋 Pending |
| Patterns page (log pattern detection) | Low | 📋 Pending |
| Live tail (SSE streaming) | Low | 🚧 In progress (endpoint shipped, dashboard not yet consuming) |
| Chart click-to-filter | Low | 📋 Planned (deferred to post-Round 15) |

## SDKs

| Language | Status |
|----------|--------|
| Node.js | ✅ Shipped |
| Python | ✅ Shipped |
| Go | ✅ Shipped |
| Rust | ✅ Shipped |

## Distribution

| Feature | Priority | Status |
|---------|----------|--------|
| npm package (CLI) | High | ✅ Done |
| Precompiled binaries (Linux, macOS) | High | ✅ Done |
| Homebrew formula | Medium | 📋 Pending |
| Windows native (non-WSL2) | Low | 📋 Pending |

## Performance (baseline)

Ingest throughput baseline (v0.1): ~16.7k ev/s single producer, ~25k ev/s multi-producer (32 producers). See [`bench/`](bench/) for methodology and machine config. Performance optimization is deferred — correctness first.
