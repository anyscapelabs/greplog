# Dashboard

The embedded dashboard lives at `http://localhost:3000`. `greplog dev` builds
and serves it automatically — no separate frontend step. In debug builds the
bundle is read from `dashboard/dist` (run `npm run build` there after UI
changes); release binaries have it baked in.

## Log Explorer

- **Timeline** histogram of ingest volume for the selected range; drag its
  bottom edge to resize.
- **Search bar** accepting free text and `field:value` terms
  (`level:error`, `service:auth-api`, `"quoted phrases"`), applied on
  **Run query**.
- **Facet checkboxes** in the left sidebar (severity, service) — check to
  filter, uncheck to remove; the active selection is highlighted.
- **Service dropdown** narrows to one service.
- Rows expand to show the `raw_body` payload.

## Metrics

Aggregations over the selected window: ingestion timeline, severity
breakdown, per-service ingestion, a service health table, the overall
**error rate**, and **storage** usage on disk.

## Live tail

Streams committed logs over Server-Sent Events — no polling. Pause/resume
and clear are per-tab; dropped batches (subscriber lag) are marked inline
rather than skipped silently.

## Controls

| Control | Effect |
|---------|--------|
| Time-range picker | 15m → 30d window for every query on the page |
| Refresh button | Refetches the active tab's queries in place |
| Auto: off/5s/10s/30s/1m | Repeats that refresh on an interval; paused while the tab is hidden |

Filters, search text, and scroll position survive refreshes.
