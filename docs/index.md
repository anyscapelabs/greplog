# Greplog Docs

Fast, lightweight, zero-data-loss logging engine and dashboard — built with Rust, Apache Arrow, DataFusion, and Vite.

## For the impatient user

- [Installation](1-getting-started/installation.md) — one-line install and basic setup
- [Dashboard](1-getting-started/dashboard.md) — using the UI on port 3000
- [Configuration](1-getting-started/configuration.md) — env vars, ports, and TTL

## For the application developer

- [Node.js / TypeScript SDK](2-sdks/nodejs.md)
- [Python SDK](2-sdks/python.md)
- [Rust SDK](2-sdks/rust.md)

## For open-source contributors and nerds

- [Zero-Loss WAL](3-architecture/zero-loss-wal.md) — group commit and fsync
- [Storage Engine](3-architecture/storage-engine.md) — MemTable → Compactor → Parquet
- [Query Engine](3-architecture/query-engine.md) — sub-second queries with DataFusion

## For community builders

- [Local Development](4-contributing/local-dev.md) — running the Cargo workspace
- [Roadmap](4-contributing/roadmap.md) — what's coming next
- [Code of Conduct](4-contributing/code-of-conduct.md)