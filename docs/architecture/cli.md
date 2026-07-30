# CLI (`greplog`)

> **Status:** ✅ Shipped (v0.1)

Three commands bundled in a single binary:

- `greplog init` — Detect framework, establish service identity, write `greplog.config.json`.
- `greplog dev` — Start local agent + serve dashboard API. Runs in background unless `--foreground` is used.
- `greplog status` — Show agent health, buffer size, disk usage, event counts.

The CLI wraps `greplog-agent` as a library. It does not vendor or embed the React dashboard — the dashboard is a separate project served independently.

## HTTP API Endpoints

> **Status:** ✅ Shipped (v0.1)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check, returns `{"status":"ok","version":"..."}` |
| `GET` | `/status` | Agent runtime status (dropped events, buffer occupancy) |
| `GET` | `/detect` | Framework workspace detection |
| `GET` | `/resources` | System resource usage (CPU, memory, disk) |
| `POST` | `/query` | Execute SQL query against stored data |
| `GET` | `/tail` | **Server-Sent Events** stream for Live Tail |🚧|

### `/query`

Accepts `{"sql": "SELECT ..."}` and returns:

```json
{
  "columns": ["column_a", "column_b"],
  "rows": [["val1", "val2"], ...],
  "row_count": 123
}
```

### `/tail` 🚧

> **Status:** 🚧 In progress (Round 14, Phase 1)

Returns a Server-Sent Events (SSE) stream of ingested events in real time. Each event is a JSON line:

- **Log:** `{"type":"log","timestamp_ns":...,"service":"...","level":"...","message":"...",...}`
- **Span:** `{"type":"span","start_time_ns":...,"service":"...","name":"...",...}`
- **Metric:** `{"type":"metric","timestamp_ns":...,"service":"...","name":"...","value":...,...}`

The stream sends a `keep-alive` comment every 15 seconds to prevent proxy timeouts. Clients connect via the browser's native `EventSource` API.

> **Design decision (v0.1):** The `/tail` endpoint streams *all* ingested events unfiltered.
> Server-side filtering (`?filter=...`) was deferred because it requires per-subscriber
> predicate state, which the current `broadcast::Sender` model doesn't support natively.
> Client-side filtering via the dashboard's filter bar is the expected path for v0.1.
