# Changelog

All notable changes to Greplog are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
adheres to [Semantic Versioning](https://semver.org).

## [Unreleased]

### Added
- Structured `POST /api/search`: validated JSON queries compiled into
  partition-pruned per-tier scans, with rows and aggregate modes and a
  global-aggregate path for error-rate cards.
- `GET /api/stats` returning Parquet disk usage (bytes, partitions, chunks).
- Python SDK (`sdk/python`): buffered client with batching, retry, queue cap.
- Rust SDK (`sdk/rust`): buffered client with level macros, batching, retry.
- Dashboard: Metrics page fully wired (severity breakdown, ingestion by
  service, service table, error rate, storage); Live Tail page streaming
  over SSE; checkbox facet filters; refresh via query invalidation.

### Changed
- Dashboard queries moved off raw SQL passthrough onto `/api/search`.
- Refresh controls no longer remount pages; refetches happen in place.

### Fixed
- DataFusion 39 mis-prunes when two Hive partition columns are conjuncted;
  searches now emit a single-clause partition filter.
- Facet picks from the sidebar translate UI names to wire columns
  (`severity` → `level`).
- Service table reads the server's `count` metric instead of a stale alias,
  and no longer shows fabricated latency/environment columns — latency comes
  from `latency_ms` in the log payload when producers include it.
- Parquet chunks are partitioned by each record's own timestamp, not the
  flush clock: late records land in the day window they belong to.
- WAL replay distinguishes a truncated crash tail from mid-file corruption
  instead of matching error strings.
- The live query buffer is capped under persistent Parquet-write failure;
  shed rows stay WAL-durable and replay on restart.
- CI builds the dashboard bundle before compiling so binaries embed it.

## [0.1.0] – initial development release

- Ingest API with WAL-backed zero-loss writes (fsync before ack, segment
  rotation on Parquet confirmation).
- Dual-tier storage: Arrow MemTable → Snappy Parquet in Hive-partitioned
  day/service directories; background compactor and retention purger.
- DataFusion query engine over a unified live + Parquet view; SSE live tail.
- Single-binary dashboard served via rust_embed.
- Node.js and Go SDKs.
