# Dashboard

The embedded Vite dashboard lives at `http://localhost:3000` and requires no separate frontend build step — `greplog dev` serves it automatically.

## What you can do

- **Live tail** incoming logs in real time over Server-Sent Events (SSE).
- **Query** your logs with the DataFusion SQL engine and see results in milliseconds.
- **Filter** by service name, environment, level, and time range.
- **Inspect** storage usage and retention status.

## Live tail

The dashboard subscribes to the SSE stream at `/api/log/stream` and renders logs the moment the WAL commits them. No polling, no refresh.

## Running queries

Use the query bar to run SQL against your logs, SQL-like ad-hoc queries should be answered sub-second even on millions of rows. For examples, see the [Query Engine](../3-architecture/query-engine.md).

## Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| `c` | Clear results |
| `f` | Focus search bar |
| `Esc` | Clear filters |