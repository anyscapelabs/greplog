# Node.js / TypeScript SDK

The `greplog` npm package (source: [`sdk/node`](../../sdk/node)) gives your app zero-friction logging with client-side batching.

## Install

The SDK is published as `greplog`; from the repo it can be used directly:

```bash
npm i /path/to/greplog/sdk/node
```

## Initialize

```typescript
import greplog from 'greplog';

// All three forms are equivalent:
greplog.init('payment-service', 'production');
greplog.init({ service: 'payment-service', env: 'production' });
greplog.init(); // reads from process.env
```

Missing values fall back to the environment:

- `GREPLOG_SERVICE_NAME` — default `"node-app"`
- `GREPLOG_ENV` — default `"development"`
- `GREPLOG_URL` — default `http://127.0.0.1:5050` (the ingest port, `POST /api/log`)

Calling `init` more than once is a no-op. It also auto-instruments the process:
`console` methods are monkey-patched (original output to stdout/stderr is preserved),
and `uncaughtException` / `unhandledRejection` are captured as `CRITICAL` records.

## Log

```typescript
greplog.info('User signed in', { userId: 'usr_123' });
greplog.error('Payment failed', { error: 'Card declined' });
```

Available levels: `trace`, `debug`, `info`, `warn`, `error`, `fatal`. The optional
second argument is serialized into the record's `raw_body` JSON payload.

## Batching

Logs are **not** sent one-by-one. The SDK buffers records in memory and flushes
them to `POST /api/log` in batches:

- Flush when the buffer reaches `batchSize` records (default 100).
- Flush on a `flushIntervalMs` interval if the buffer never fills (default 500ms).
- Configurable, non-blocking: network sends are fire-and-forget and never block the event loop.

## Graceful failure

If the Greplog backend is unreachable, the SDK silently re-queues the batch and
keeps trying — it never throws into your application. The in-memory buffer is
capped at `maxQueueSize` records (default 10,000); the oldest records are dropped
beyond that so process memory stays bounded.

## Graceful shutdown

```typescript
await greplog.flush();
```

Call this before your process exits to guarantee all buffered logs are sent.
The flush timer is `unref()`-ed, so it never keeps the process alive on its own.

## Configuration reference

```typescript
greplog.init({
  service: 'payment-service',
  env: 'production',
  endpoint: 'http://127.0.0.1:5050', // base URL, /api/log is appended
  batchSize: 500,
  flushIntervalMs: 1000,
  maxQueueSize: 50000,
});
```