# Greplog Dashboard

The Greplog web dashboard — a Vite + React + TypeScript frontend styled with
Tailwind CSS v4, served by `greplog dev` on port 3000.

## Development

```bash
npm install
npm run dev      # start the Vite dev server
npm run build    # type-check and produce dist/
npm run lint     # oxlint
```

## Planned features

- **Live tail** of incoming logs over Server-Sent Events (SSE).
- **Query** logs with the DataFusion SQL engine.
- **Filter** by service name, environment, level, and time range.

See [`docs/1-getting-started/dashboard.md`](../docs/1-getting-started/dashboard.md).
