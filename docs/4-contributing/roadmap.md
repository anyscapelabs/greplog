# Roadmap

Ordered by rough priority. Community input is welcome — open an issue or a discussion to weigh in.

## Near term

- **Full-text search with Tantivy**: add a Tantivy index layer for fast free-text queries over log messages, complementing DataFusion's structured SQL.
- **Multi-instance streaming** (replication / follower re-sync from `current.wal`).
- **Log correlation** — trace/span propagation hints surfaced in the dashboard.
- **Alerting** — threshold and anomaly rules evaluated against the query engine.

## Mid term

- **Parquet partitioning by service** in addition to the current day-based layout.
- **Query snapshots / saved queries** in the dashboard.
- **Namespaces and teams** with read-only share links.
- **Vector/tracing ingestion** alongside the JSON log API.

## Longer term

- **Native clustering** — shared storage backend while keeping the single-binary story.
- **Plugin SDKs** for more languages and observability formats (OpenTelemetry exporter).
- **Blazing-fast retention-aware compactor** with refreshable page indexes.

## How to propose a feature

1. Search existing issues first.
2. Open an issue with the problem, the use case, and (optionally) a sketch.
3. For design-heavy changes, start a discussion before opening a PR.