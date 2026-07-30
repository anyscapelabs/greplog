# greplog-agent

Local observability daemon — ingests, stores, and serves observability data.

## What this is

The single Rust binary that receives events from SDKs over UDS/TCP, writes them through a crash-safe WAL and content-addressed dedup, accumulates them in an Arrow buffer, flushes to partitioned Parquet files, and serves the dashboard via an embedded Axum HTTP server. Runs as a local daemon per workspace.

## Building

```sh
cargo build -p greplog-agent
```

## Testing

```sh
cargo test -p greplog-agent
```

Integration tests are in `tests/modules/` as extracted submodules from the original inline test modules.

## Structure

```
src/
├── ingest/       — UDS & TCP listeners, batching channels
├── store/        — WAL, dedup, Arrow buffer, Parquet flush, compaction
├── server/       — embedded HTTP server (Axum) + static dashboard
├── detect/       — workspace framework auto-detection
├── query_engine.rs — Parquet/Arrow query layer
├── lib.rs        — Config, run(), module tree
└── main.rs       — binary entry point
tests/modules/    — extracted test modules (wal, dedup, buffer, compaction, etc.)
```

## Relationship to the rest of greplog

The agent is the central process — all four SDKs connect to it, the CLI controls it, and the dashboard renders its data. See [docs/architecture/overview.md](/docs/architecture/overview.md).
