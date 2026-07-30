# Greplog Dashboard

React + TypeScript dashboard embedded in the greplog agent binary.

## What this is

A single-page React app served by the agent at `localhost:4317`. Displays a unified timeline of logs, errors, and HTTP calls across all services. Built with Vite and consumes the agent's `/query`, `/resources`, and `/detect` HTTP endpoints.

## Building

```sh
cd dashboard && npm install && npm run build
```

Output goes to `dist/`, which is embedded into the agent binary at build time via `rust-embed`.

## Testing

```sh
cd dashboard && npm test
```

## Structure

```
src/     — React components, pages, hooks, and styles
dist/    — compiled output (embedded in agent binary)
```

## Relationship to the rest of greplog

The dashboard is served by `greplog-agent`'s embedded HTTP server. It is not a standalone app — it must be built before the agent binary is compiled, or the agent serves a 404 for the UI route. See [docs/architecture/dashboard.md](/docs/architecture/dashboard.md).
