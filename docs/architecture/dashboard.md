# Dashboard (React)

> **Status:** Dashboard UI is functional with real agent API queries. Filter state is synced to URL search params. Service drawer shows real errors/logs per service. Sparklines wired from time-bucketed queries. Chart click-to-filter is deferred.

## Unified Timeline

The core UX revolves around interleaved logs:

- **Interleaved Log Explorer:** A single, chronological stream of all events from all connected services in the workspace. Sorting by timestamp ASC naturally aligns requests and responses across services.
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

## Toast Notifications

Failed requests are no longer silent. The §0 wiring-audit fix (failed `/query`
surfaces a toast instead of being masked by placeholder-data fallback) and the
toast mechanism that renders it are built together:

- **One shared mechanism** — `ToastProvider`/`useToast()` wraps the app in
  `main.tsx`, alongside `ThemeProvider`/`QueryProvider`. Every consumer calls
  `useToast()` (or the store directly) — no per-component toast implementations.
  The underlying logic lives in `src/lib/toastStore.ts` as a module singleton,
  so non-React code (`hooks/api.ts`, `context/AgentContext.tsx`) can raise
  toasts without a component; React renders it via `useSyncExternalStore`.
- **Placement/visual** — fixed top-right, newest-on-top stack, `--error`/
  `--success` tokens (theme-aware), icon + message + dismiss button, and an
  auto-dismiss progress bar (thin shrinking bar matching the dismiss timer;
  persistent toasts show no bar). Container is `aria-live="polite"`; error
  toasts get `role="alert"`.
- **Timing** — success auto-dismisses after 4s, error after 8s. `durationMs: 0`
  marks an error as ongoing-state (e.g. "Agent unreachable") requiring manual
  dismissal.
- **Anti-spam (load-bearing)** — see the dedupe/rate-limit rules in
  `toastStore.ts`. Background failures (`postQuery`, agent health polling) carry
  a `dedupeKey` and are deduped (never two identical toasts) and rate-limited to
  at most one re-toast per 60s while the failure persists. User-initiated action
  failures pass no key and always show. Recovery is keyed: `showSuccess(msg,
  { dedupeKey })` only surfaces when that key is in an error state — closing the
  loop ("Query succeeded again" / "Reconnected to agent") instead of the error
  silently expiring.
- **Triggers today** — any failed `/query` (via `postQuery`), agent
  disconnection mid-session (persistent error toast, distinct from the
  onboarding `WaitingOverlay`), and recovery from either. Not toasting: every
  successful background poll, initial onboarding, clear-filters (self-evident
  from the UI). Export/CSV has no UI yet — when it lands it should call
  `showError`/`showSuccess` directly (no dedupe key) for always-show semantics.
- **Tests** — `test/toastStore.test.ts` covers dedupe, cooldown re-toasting,
  recovery transitions, persistent toasts, and snapshot stability, run via
  `npm test` (Node's built-in runner, injected fake clock/timers — no deps).

> **Status:** ✅ Shipped

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

### HTTP metrics: dual source (spans + log attributes)

HTTP request/response data is captured **differently per SDK** (see the SDK
wire-format reconciliation item in `ROADMAP.md`):

- **Go and Rust SDKs** send HTTP metadata as `Span` messages → `spans` table.
- **Node.js and Python SDKs** send it as `LogEvent.attributes` (keys `http.method`,
  `http.route`, `http.status_code`, `http.latency_ms`, `logger_name = "greplog.http"`)
  → `logs` table, extracted with DataFusion's `json_get_str(attributes, 'http.latency_ms')`.

The three HTTP charts query **both** sources as a single `UNION ALL` per chart
(`useAnalytics` runs 9 queries total). Each source's rows feed the same
aggregation, so percentiles, sums and counts are computed over the true combined
population server-side — no client-side merging:

- **LatencyPercentiles** — `SELECT approx_percentile_cont(latency_ms, …) FROM (SELECT latency_ms FROM spans <where> UNION ALL SELECT CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) FROM logs WHERE logger_name = 'greplog.http' <where>) t`.
- **StatusCodes** — per-source `GROUP BY` counts, then a second `GROUP BY status_code` summing across both arms.
- **AvgResponseTime** — both arms project `service` + `latency_ms`; the outer query returns `SUM(latency_ms)` + `COUNT(*)` per service (weighted average is exact even when a service has rows from both sources).

Each arm gets its **own translation of the user's filter predicate**
(`httpArmPredicates` in `src/lib/httpPredicates.ts`, used by `useAnalytics.ts`),
so a filter narrows **both** arms
the same way — a filter that only applied to one arm would aggregate over a
mathematically wrong mixed population. Translation rules:

- `service IN (...)`, `correlation_id = 'x'` — unchanged on both arms.
- `timestamp op N` (the time-range filter) — `timestamp` on the logs arm,
  `start_time` on the spans arm (the spans table has no `timestamp` column).
- `level IN (...)`, `message LIKE '%x%'`, `line op N` — the `logs`-only
  columns, translated per arm:
  - **`level`**: the Node.js/Python HTTP middleware derives `level` from the
    response status (`>=500` → error, `400-499` → warn, `<400` → info), so for
    HTTP rows `level` IS a status bucket. The logs arm keeps the raw `level`
    clause (equivalent by that mapping); the spans arm translates it to a
    `status_code` predicate (e.g. `level IN ('error','critical','fatal')` →
    `status_code >= 500`; `warn` → `400 <= status_code < 500`; `info` →
    `status_code < 400`; `debug` and other levels the middleware never emits
    match nothing on both arms).
  - **`message`**: the logs arm keeps `message LIKE`; the spans arm maps it to
    `name LIKE '%x%' OR route LIKE '%x%'` (span name is `"METHOD route"`).
  - **`line`** (status chips compile to `line` predicates): `line` is null on
    HTTP rows, so both arms filter on `status_code` instead — the logs arm via
    `CAST(json_get_str(attributes, 'http.status_code') AS INT)`.
- Unrecognized clause shapes — fail loudly (AGENTS.md Rule 8): the three HTTP
  queries are skipped and a console warning names the unhandled clause, so a
  future filter type must be added as an explicit case in `httpArmPredicates`
  instead of silently degrading to a skewed population.

The translation helpers are pure functions extracted into
`src/lib/httpPredicates.ts` and covered by `test/httpPredicates.test.ts`
(`npm test`, run with Node's built-in test runner — no extra dependencies).
The regression cases include the correlation_id matcher being bounded to a
single pre-split clause (the greedy `.*` must not swallow a following
predicate) and unrecognized shapes landing in `unsupported` rather than being
silently ignored.

`json_get_str` returns NULL for missing keys/malformed JSON, so bad rows are
skipped by the aggregates rather than failing the query.

Empty charts for these three show an SDK-version message ("No HTTP metrics —
request data depends on SDK capture…") rather than the generic not-connected
state.

### Analytics Charts (wired status)

| Chart | Source | Status |
|-------|--------|--------|
| Log Ingestion Over Time | `logs` table, `GROUP BY date` | ✅ Wired |
| Error Rate Over Time | `logs` table, error-filtered | ✅ Wired |
| Latency Percentiles | `spans` + `logs.attributes` (`json_get_str`) `UNION ALL`, `approx_percentile_cont` | ✅ Wired |
| Service Health | `logs` table, per-service error breakdown | ✅ Wired |
| Status Codes | `spans` + `logs.attributes` (`json_get_str`) `UNION ALL`, grouped counts | ✅ Wired |
| Top Noisy Services | `logs` table, top 5 by count | ✅ Wired |
| Log Severity Distribution | `logs` table, `GROUP BY level` | ✅ Wired |
| Avg Response Time | `spans` + `logs.attributes` (`json_get_str`) `UNION ALL`, `SUM`/`COUNT` per service | ✅ Wired |
| System Metrics | OS-level agent collection needed | 📋 Blocked |

> **Status:** ✅ 8 of 9 Analytics charts wired. System Metrics blocked on new agent capability.
