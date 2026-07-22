# Resolved Architectural Decisions (Locked In)

- **Wire format:** Protobuf. Safer default for ecosystem support across Go/Python/Node/Rust and allows strict schema evolution.
- **Windows support:** v1 is Unix-only (macOS/Linux + WSL2). Native Windows named pipes are deferred.
- **Agent Scope:** One agent per workspace (monorepo multiplexing), binding to a local `.greplog/greplog.sock` and local TCP fallback port.
- **Distributed Tracing:** Deferred. v1 relies on chronological interleaving (pseudo-tracing) + optional manual `correlation_id` to keep SDKs thin and stable. W3C trace propagation will come in a later phase.
- **Storage Backend:** Apache Arrow in-memory record batches, flushed to partitioned Parquet files. No embedded OLAP database. Arrow was chosen over DuckDB for zero-copy query paths, direct integration with the Parquet crate, and elimination of the SQL translation layer for dashboard queries. Read concurrency is handled by the query engine (parquet + Arrow buffer readers).
- **Crash Safety:** Write-ahead log (WAL) before acknowledgment. On recovery, the WAL is replayed through dedup → buffer → flush. This replaces the earlier approach of relying on the filesystem write cache.
- **Dedup:** Content-addressed dedup (BLAKE3) at the event level. SDKs are idempotent but the agent is the single source of truth for dedup decisions, keeping SDKs stateless.
- **Compaction:** Background task merging small Parquet partitions. Idempotent by design — partial merges are discarded on crash. Skips the current hour's partition to avoid flush contention.
- **SDK Startup:** Fail-open with exactly one warning log, then silent drops.
