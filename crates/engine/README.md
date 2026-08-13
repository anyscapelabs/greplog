# greplog-engine

The core logging engine crate for Greplog.

## Responsibilities

- `error.rs` — the crate-wide `EngineError` (Arrow, Parse, Io, Bincode, QueryRejected, QueryTimeout).
- `record.rs` — the strongly-typed `LogRecord` wire model.
- `schema.rs` — the canonical Arrow schema with dictionary-encoded `level`/`service`.
- `ingest.rs` — the `IngestBatch` unit of work with durability responder.
- `wal.rs` — the physical Write-Ahead Log writer (bincode + flush + `fsync`).
- `memtable.rs` — the Arrow columnar buffer (array builders + `RecordBatch`).
- `worker.rs` — the dedicated WAL + MemTable OS threads, joined by a lock-free
  `crossbeam` handoff so the MemTable only sees durable records.
- `compactor.rs` — the background compactor merging crowded Parquet partitions.
- `retention.rs` — the background TTL purger removing expired day partitions.
- `query.rs` — the DataFusion-backed `QueryEngine` over a unified `logs` view of
  live + Parquet data, with read-only statement validation, query timeouts, and
  result row caps.

## Development

```bash
cargo build -p greplog-engine
cargo clippy -p greplog-engine --all-targets
cargo test -p greplog-engine
```

The crate enforces `#![warn(clippy::all, clippy::pedantic)]` and forbids panics
in production code — all errors flow through `EngineError` with `?`.