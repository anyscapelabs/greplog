# The Local Agent (The Monorepo Multiplex)

This is what makes `greplog dev` feel instant. It's a single Rust binary, precompiled per platform.

## Workspace Scoping

When `greplog dev` runs, it drops a socket at `.greplog/greplog.sock` in the root of the project. This allows multiple services in a monorepo to stream to the same agent. If you open a completely separate project and run `greplog dev` there, it spawns a second isolated agent on a different UI port (e.g., 4318).

## UDS + TCP Fallback

Listens on the Unix domain socket and a local TCP port (e.g., `127.0.0.1:4318`). Host apps connect via UDS (zero config). Dockerized apps fail over to the TCP port seamlessly to escape the container boundary.

## Single-Threaded Writer Loop

Incoming events are batched from the UDS/TCP ingest workers and passed via a channel to a single dedicated DuckDB writer thread. This prevents SQLite/DuckDB file-locking panics while allowing the HTTP API to read concurrently.

## Batches & Writes

Flushes the buffer into DuckDB (in-memory + WAL) every 2s, with periodic compaction to Parquet files on disk (`~/.greplog/<project>/logs/*.parquet`), partitioned by hour.

## Serves Queries & Dashboard

The dashboard doesn't talk SQL directly; the agent exposes a small HTTP query API (`POST /query` → translates to SQL against DuckDB). The React build is embedded into the binary and served on `http://localhost:<port>`.
