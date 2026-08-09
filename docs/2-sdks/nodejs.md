# Node.js / TypeScript SDK

The `greplog` npm package gives your app zero-friction logging with client-side batching.

## Install

```bash
npm i greplog
```

## Initialize

```typescript
import greplog from 'greplog';
greplog.init('user-service', 'production'); // Or greplog.init() to read from process.env
```

`init(serviceName, env)` accepts the service name and environment explicitly, or reads them from `GREPLOG_SERVICE_NAME` and `GREPLOG_ENV` when called with no arguments. The backend URL comes from `GREPLOG_URL` (default `http://localhost:3000`).

## Log

```typescript
greplog.info('User signed in', { userId: 'usr_123' });
greplog.error('Payment failed', { error: 'Card declined' });
```

Available levels: `trace`, `debug`, `info`, `warn`, `error`, `fatal`.

## Batching

Logs are **not** sent one-by-one. The SDK buffers records in memory and flushes them to `POST /api/log` in batches:

- Flush when the buffer reaches ~1000 records.
- Flush on an ~5 second interval if the buffer never fills.
- Flush on process exit to avoid losing queued logs.

This keeps network overhead low while ensuring bounded latency.

## Graceful shutdown

```typescript
await greplog.flush();
```

Call this before your process exits to guarantee all buffered logs are sent.