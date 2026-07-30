# @greplog/node

Node/TypeScript SDK for Greplog — `import greplog from 'greplog'; greplog.init();` for auto-instrumentation.

## What this is

Monkey-patches Node's core `http` module for automatic request capture, hooks `console.error`/`console.warn` for log forwarding, and captures `uncaughtException`/`unhandledRejection`. Fail-open guaranteed — if the agent isn't running, all calls silently no-op.

## Building

```sh
cd sdks/node && npm run build
```

## Testing

```sh
cd sdks/node && npm test
```

Tests cover fail-open behavior, HTTP capture, manual API, and redaction.

## Structure

```
src/     — TypeScript source (index.ts, patchers.ts, transport.ts, redact.ts, etc.)
dist/    — compiled JavaScript
tests/   — Vitest test files
proto/   — events.proto (reference copy)
```

## Relationship to the rest of greplog

One of four SDKs that connect to the greplog agent. See [docs/sdk/design.md](/docs/sdk/design.md) for the shared contract.
