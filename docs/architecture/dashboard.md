# Dashboard (React)

A single React component library with one build target: embedded local (binary).

## Unified Timeline

The core UX revolves around interleaved logs:

- **Interleaved Log Explorer:** A single, chronological stream of all events from all connected services in the workspace. Sorting by timestamp ASC naturally aligns frontend requests, backend receptions, and DB queries.
- **Service Badges:** Every log row features a distinct color tag based on its `service_name` (e.g., a green `[web]` badge, blue `[api]` badge).

## Global Filter Bar

A prominent toggle to view all services or isolate a specific one, compiled into a simple `WHERE service_name = 'X'` DuckDB query. Chips for `route:/payments/*`, `status:>=500`, etc.

## Saved Views

Filter definitions stored as JSON (`~/.greplog/views.json`).

## Graphs

Latency percentiles and error-rate over time, computed via DuckDB aggregates.
