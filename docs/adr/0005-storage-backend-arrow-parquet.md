# ADR-0005: Storage Backend — Apache Arrow + Parquet

**Status:** Accepted
**Date:** 2026-03-01

## Context

The agent needs an in-memory buffer for recent events (for low-latency query) and a persistent on-disk format. An embedded OLAP database (DuckDB, SQLite) would introduce a heavy dependency and a SQL translation layer.

## Decision

Use Apache Arrow record batches for the in-memory buffer, flushed to partitioned Parquet files on disk. No embedded database.

## Alternatives considered

- **DuckDB:** Adds a SQL translation layer between the agent's query engine and storage; heavier dependency chain.
- **SQLite:** Row-oriented; poor columnar scan performance for dashboard aggregation queries.
- **Custom binary format:** More control but no ecosystem tooling for Parquet (Pandas, DuckDB can read the files directly).

## Consequences

- Zero-copy reads from the Arrow buffer into the query engine.
- Parquet files are directly readable by Pandas, DuckDB, and other tools for debugging.
- No SQL translation layer needed — the query engine works natively with Arrow and Parquet APIs.
- Full support in the Rust ecosystem via `arrow`, `parquet`, and `datafusion` crates.
