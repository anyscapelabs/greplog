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
| Interleaved log explorer | High | 🚧 In progress (mock data, awaiting real API) |
| Global filter bar | High | ✅ Done |
| Connect to real agent API | High | 🚧 In progress |
| Errors page | Medium | 📋 Pending |
| Traces page | Medium | 📋 Pending |
| Views page (saved filters) | Medium | 📋 Pending |
| Services page (per-service overview) | Medium | 📋 Pending |
| Patterns page (log pattern detection) | Low | 📋 Pending |
| Live tail (SSE streaming) | Low | 🚧 In progress (Round 14) |

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
