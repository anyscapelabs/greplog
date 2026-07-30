# Dashboard (React)

> **Status:** Dashboard is designed but currently runs on mock data. Connecting to the real agent API is **in progress**.

## Unified Timeline

The core UX revolves around interleaved logs:

- **Interleaved Log Explorer:** A single, chronological stream of all events from all connected services in the workspace. Sorting by timestamp ASC naturally aligns frontend requests, backend receptions, and DB queries.
- **Service Badges:** Every log row features a distinct color tag based on its `service_name` (e.g., a green `[web]` badge, blue `[api]` badge).

> **Status:** ✅ Shipped (v0.1) — UI rendering complete, currently powered by placeholder data.

## Global Filter Bar

A prominent toggle to view all services or isolate a specific one, compiled into a filter predicate (`service_name = 'X'`) pushed down to the agent's query engine. Chips for `route:/payments/*`, `status:>=500`, etc.

> **Status:** ✅ Shipped (v0.1)

## Saved Views

Filter definitions stored as JSON (`~/.greplog/views.json`).

> **Status:** 📋 Planned — not started.

## Graphs

Latency percentiles and error-rate over time, computed by the agent's query engine via Parquet/Arrow scans (no external database).

> **Status:** 📋 Planned — depends on metric aggregation landing in the agent (Medium priority, pending).
