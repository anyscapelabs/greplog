# Dashboard (React)

> **Status:** Dashboard UI is functional with real agent API queries. Filter state is synced to URL search params. Service drawer shows real errors/logs per service. Sparklines wired from time-bucketed queries. Chart click-to-filter is deferred.

## Unified Timeline

The core UX revolves around interleaved logs:

- **Interleaved Log Explorer:** A single, chronological stream of all events from all connected services in the workspace. Sorting by timestamp ASC naturally aligns frontend requests, backend receptions, and DB queries.
- **Service Badges:** Every log row features a distinct color tag based on its `service_name` (e.g., a green `[web]` badge, blue `[api]` badge).

> **Status:** ✅ Shipped (v0.1) — UI rendering complete, currently powered by placeholder data.

## Global Filter Bar

A prominent toggle to view all services or isolate a specific one, compiled into a filter predicate (`service_name = 'X'`) pushed down to the agent's query engine. Chips for `route:/payments/*`, `status:>=500`, `correlation_id:X`, etc.

Filter state is synced to URL search params (`?q=`, `?c=`, `?s=`, `?t=`, `?l=`, `?ch=`) via `useFilterState()`, a React hook around `useSearchParams`. The `compileFilterToQuery()` function translates the current filter state into a SQL WHERE clause sent to the agent's `POST /query` endpoint.

- **Logs page**: Search + sidebar filters + time range → `useLogs(whereClause)`
- **Errors page**: Error-level base filter + user filters → `useErrors(whereClause)`
- **Services page**: Sidebar service checkboxes + health status → client-side union with `useServices()`
- **Analytics page**: Service dropdown + time range → `FilterState.services` for query scoping

Service filtering is unified: sidebar checkboxes and `service:` chips both write to the same `FilterState.services` array. The top-bar service dropdown was removed from Logs and Errors pages; it remains on Analytics as the authoritative service selector.

### Service Drawer

The Service Drawer (shown when clicking a service row) includes:

- **Metrics cards** — error rate, event count, health status (real data from health query)
- **Health Timeline** — 24-hour timeline (same as before)
- **Service Details** — ID (service name), Environment, Host (unavailable), Version (unavailable), Streaming since (from `MIN(timestamp)` proxy), Last deployed, First seen
- **Recent Errors** — top 5 errors for this service from `useErrors("service = 'X'")`
- **Related Logs** — top 5 logs for this service from `useLogs("WHERE service = 'X'")`

Host and Version are blocked on a cross-SDK protocol handshake (new `IngestBatch` or connection-time fields). The "Streaming since" field is honest about being a log-derived proxy, labeled explicitly.

> **Status:** ✅ Shipped (v0.1)

## Live Tail

SSE endpoint at `GET /tail` streams new log entries in real time. The dashboard's Logs page has a "Live" toggle button (UI wired, polling-driven).

> **Status:** ✅ Shipped (v0.1)

## Saved Views

Filter definitions stored as JSON (`~/.greplog/views.json`). FilterState's URL-param encoding naturally supports serialization.

> **Status:** 📋 Planned — not started.

## Graphs

Latency percentiles and error-rate over time, computed by the agent's query engine via Parquet/Arrow scans (no external database). Chart click-to-filter is deferred to a future round.

Per-service sparklines on Service Cards use the same time-bucketed query pattern as the Analytics ingestion chart, scoped per service via an existing `service` filter. The time bucket is 1 minute (`FLOOR(timestamp / 60000000)`) and sparklines are grouped client-side.

Service Drawer Recent Errors and Related Logs sections reuse `useErrors()` and `useLogs()` hooks with a service-scoped WHERE clause — no separate query path.

### Spans Table

The `spans` table is fully implemented and queryable:
- **13-column Arrow schema**: `id`, `correlation_id`, `parent_correlation_id`, `service`, `name`, `route`, `method`, `status_code` (Int32), `latency_ms` (Float64), `is_error` (Boolean), `start_time`/`end_time` (TimestampMicrosecond), `attributes`
- Ingested via the same protobuf pipeline as logs, flushed to Parquet with Hive partitioning by `service`/`date`
- Queryable via the same `/query` endpoint — `FROM spans` works identically to `FROM logs`
- `approx_percentile_cont` is a built-in DataFusion aggregate function, available out of the box (no custom UDAF registration needed)

Three Analytics charts depend on spans (LatencyPercentiles, StatusCodesPie, AvgResponseTime) — all now wired and live.

### Analytics Charts (wired status)

| Chart | Source | Status |
|-------|--------|--------|
| Log Ingestion Over Time | `logs` table, `GROUP BY date` | ✅ Wired |
| Error Rate Over Time | `logs` table, error-filtered | ✅ Wired |
| Latency Percentiles | `spans` table, `approx_percentile_cont` | ✅ Wired |
| Service Health | `logs` table, per-service error breakdown | ✅ Wired |
| Status Codes | `spans` table, `GROUP BY status_code` | ✅ Wired |
| Top Noisy Services | `logs` table, top 5 by count | ✅ Wired |
| Log Severity Distribution | `logs` table, `GROUP BY level` | ✅ Wired |
| Avg Response Time | `spans` table, `AVG(latency_ms)` per service | ✅ Wired |
| System Metrics | OS-level agent collection needed | 📋 Blocked |

> **Status:** ✅ 8 of 9 Analytics charts wired. System Metrics blocked on new agent capability.
