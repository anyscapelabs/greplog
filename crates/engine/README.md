# greplog-engine

The core logging engine crate for Greplog.

## Responsibilities

- `error.rs` — the crate-wide `EngineError` (Arrow, Parse, Io, Bincode).
- `record.rs` — the strongly-typed `LogRecord` wire model.
- `schema.rs` — the canonical Arrow schema with dictionary-encoded `level`/`service`.
- `ingest.rs` — the `IngestBatch` unit of work with durability responder.
- `wal.rs` — the physical Write-Ahead Log writer (bincode + flush + `fsync`).
- `worker.rs` — the dedicated OS thread draining the ingest channel.

## Development

```bash
cargo build -p greplog-engine
cargo clippy -p greplog-engine --all-targets
cargo test -p greplog-engine
```

The crate enforces `#![warn(clippy::all, clippy::pedantic)]` and forbids panics
in production code — all errors flow through `EngineError` with `?`.