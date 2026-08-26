# Changelog

All notable changes to Greplog are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
adheres to [Semantic Versioning](https://semver.org).

## [0.1.0] – 2026-08-26

First public release.

### Added
- Ingest API with WAL-backed zero-loss writes (fsync before ack, segment
  rotation on Parquet confirmation).
- Dual-tier storage: Arrow MemTable → Snappy Parquet in Hive-partitioned
  day/service directories; background compactor and retention purger.
- DataFusion query engine over a unified live + Parquet view; SSE live tail.
- Structured `POST /api/search`: validated JSON queries compiled into
  partition-pruned per-tier scans, with rows and aggregate modes and a
  global-aggregate path for error-rate cards.
- `GET /api/stats` returning Parquet disk usage (bytes, partitions, chunks).
- Single-binary dashboard served via rust_embed: Log Explorer with facet
  filters, Metrics page (severity breakdown, ingestion by service, service
  table, error rate, storage), and Live Tail streaming over SSE.
- `greplog uninstall`: removes storage, WAL and the binary in one confirmed
  step, reporting sizes and warning when an instance is still serving.
- `--host` flag on `greplog dev` and `greplog start` to choose the bind
  interface.
- SDKs: Node.js, Go, Python (buffered client with batching, retry, queue
  cap), Rust (buffered client with level macros).

### Changed
- Both servers bind `127.0.0.1` by default instead of every interface;
  pass `--host 0.0.0.0` to accept traffic from other machines.
- Storage moved from working-directory-relative `data/` to a fixed per-user
  location — `~/.local/share/greplog` (Linux), `~/Library/Application
  Support/greplog` (macOS), `%APPDATA%\greplog` (Windows) — overridable with
  `GREPLOG_DATA_DIR`, so every command sees the same data regardless of
  where it runs. Move an old `data/logs` and `data/wal` into the new
  location to keep previously ingested logs.
- Dashboard queries moved off raw SQL passthrough onto `/api/search`.
- Refresh controls no longer remount pages; refetches happen in place.

### Fixed
- Ingest rejects service names that could alter the storage layout
  (`service` becomes a `service=<name>` directory, so `/` or `..` in the
  name could write outside the data directory). Valid names are 1–64
  characters of `a-z A-Z 0-9 _ . -`; offending batches get a 400.
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
