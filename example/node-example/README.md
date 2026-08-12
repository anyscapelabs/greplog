# Greplog Node.js Example

This is a minimal Express.js application demonstrating the zero-configuration auto-instrumentation of the Greplog SDK. All log statements are plain `console` calls — no explicit SDK imports are needed in the route files.

## Running the Example

1. Ensure the Greplog engine is running in a separate terminal:
   ```bash
   cargo run -p greplog-cli dev
   ```

2. Install dependencies and start the Express server:
   ```bash
   npm install
   npm start
   ```

3. Send a test request to see the logs instantly appear in the Greplog dashboard:
   ```bash
   curl -X POST http://localhost:4000/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"email":"test@example.com", "password":"wrong"}'
   ```

You should see the `warning` for the failed login appear in the dashboard under the
`api-gateway` service.

## Try the other routes

```bash
# Successful login
curl -X POST http://localhost:4000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"test@example.com", "password":"secure_password_123"}'

# Successful order
curl -X POST http://localhost:4000/api/orders \
  -H "Content-Type: application/json" \
  -d '{"items":[{"sku":"a1"},{"sku":"b2"}], "total_price":49.99, "user_id":"usr_98765"}'

# Failed order (throws, stack trace captured automatically)
curl -X POST http://localhost:4000/api/orders \
  -H "Content-Type: application/json" \
  -d '{"items":[]}'
```

## How it works

- `greplog.init(...)` at the top of `src/index.js` boots the SDK and monkey-patches
  `console.log/info/warn/error/debug`, preserving normal output to stdout/stderr.
- Logs are buffered and flushed in batches to the Greplog ingest endpoint
  (`http://127.0.0.1:5050` by default). Override with `GREPLOG_URL`.
- Structured arguments passed to `console.info`/`console.warn` become the
  `raw_body` JSON payload on each record.
- `uncaughtException` and `unhandledRejection` are captured automatically as
  `CRITICAL` records.

## Configuration

| Env var | Default | Purpose |
| --- | --- | --- |
| `PORT` | `4000` | Express listen port |
| `GREPLOG_SERVICE_NAME` | `api-gateway` (set in code) | Service label in the dashboard |
| `GREPLOG_ENV` | `development` (set in code) | Deployment environment |
| `GREPLOG_URL` | `http://127.0.0.1:5050` | Greplog ingest server base URL |