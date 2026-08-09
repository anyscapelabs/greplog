# Configuration

Greplog is configured through CLI flags and environment variables.

## CLI flags

| Flag | Default | Description |
|------|---------|-------------|
| `--port` | `3000` | HTTP port for the ingest server and dashboard |
| `--retention-days` | `30` | How long Parquet data is kept before automatic purge |
| `--data-dir` | `~/.greplog` | Where WAL and Parquet files are stored |

### Port overrides

To run Greplog behind another process or on a privileged port:

```bash
greplog start --port 5050
```

SDKs must then point at the new URL:

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
| `GREPLOG_URL` | Greplog backend server URL | `http://localhost:3000` |
| `GREPLOG_SERVICE_NAME` | Name of your microservice/app | `api-gateway` |
| `GREPLOG_ENV` | Deployment environment | `production` |

### SDK example

```bash
export GREPLOG_URL=http://localhost:3000
export GREPLOG_SERVICE_NAME=api-gateway
export GREPLOG_ENV=production

# SDKs now initialize with zero arguments
greplog.init()
```