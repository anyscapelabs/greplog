# Configuration

Greplog is configured through CLI flags and environment variables.

## CLI flags

| Flag | Default | Description |
|------|---------|-------------|
| `--port` | `3000` | Dashboard API port (`/api/query`, `/api/tail`). The ingest API is fixed on `5050`. |
| `--retention-days` | `30` | How long Parquet data is kept before automatic purge |

### Port overrides

The `--port` flag moves the dashboard API; the ingest API always stays on `5050`:

```bash
greplog start --port 8080
```

SDKs must then point at the ingest URL (still `5050` unless proxied):

```bash
export GREPLOG_URL=http://localhost:5050
```

### TTL / retention

Parquet directories older than `--retention-days` are purged automatically. There is no heavy SQL DELETE — old files are simply removed from disk.

```bash
greplog start --retention-days 7
```

## Environment variables

These are read by the SDKs (`greplog.init()` with no arguments) and by the server.

| Variable | Description | Example |
|----------|-------------|---------|
| `GREPLOG_URL` | Greplog ingest server URL (default `http://127.0.0.1:5050`) | `http://localhost:5050` |
| `GREPLOG_SERVICE_NAME` | Name of your microservice/app | `api-gateway` |
| `GREPLOG_ENV` | Deployment environment | `production` |

### SDK example

```bash
export GREPLOG_URL=http://localhost:5050
export GREPLOG_SERVICE_NAME=api-gateway
export GREPLOG_ENV=production

# SDKs now initialize with zero arguments
greplog.init()
```