<p align="center">
  <img src="assets/branding/logo/wordmark/wordmark-white.svg" alt="Greplog" width="400">
</p>

> Fast, lightweight, zero-data-loss logging engine and dashboard for solo developers, startups, and small teams. Built with Rust, Apache Arrow, DataFusion, and Vite.

Greplog replaces clunky, resource-heavy cloud observability stacks with a single binary that handles ingestion, storage, real-time streaming, and querying—all locally or on a single small server.

---

## Quick Start

### 1. Installation

Install the `greplog` CLI with a single command:

```bash
curl -fsSL https://greplog.dev/install.sh | sh
```

### 2. Start Greplog

Run the dev command in your terminal:

```bash
greplog dev
```

This spins up:

- 🟢 **Ingest Server** at http://localhost:3000/api/log
- 📊 **Embedded Vite Dashboard** at http://localhost:3000
- ⚡ **DataFusion Query Engine** over local Parquet storage

### 3. SDK Setup (2-Line Config)

Greplog SDKs use zero-friction initialization. Environment and service parameters can be passed directly or read automatically from environment variables (`GREPLOG_SERVICE_NAME` and `GREPLOG_ENV`).

#### Node.js / TypeScript

```bash
npm i greplog
```

```typescript
import greplog from 'greplog';
greplog.init('user-service', 'production'); // Or greplog.init() to read from process.env

// Use standard logging
greplog.info('User signed in', { userId: 'usr_123' });
greplog.error('Payment failed', { error: 'Card declined' });
```

#### Python

```bash
pip install greplog
```

```python
import greplog

greplog.init("payment-service", "production")  # Or greplog.init() to read from os.environ

# Use standard logging
greplog.info("Processing order", {"order_id": 9876})
greplog.error("Database connection lost", {"retry_count": 3})
```

#### Rust

```bash
cargo add greplog
```

```rust
use greplog;

fn main() {
    greplog::init("auth-service", "production"); // Or greplog::init_from_env()

    greplog::info!("User authenticated", user_id = 42);
    greplog::error!("Token generation failed", reason = "Key expired");
}
```

## How Greplog Works

Greplog is designed around a **Group Commit Pipeline** and **Dual-Tier Compaction** to guarantee zero data loss while keeping CPU usage under 3%.

```
[SDK Ingest] ➔ [Axum Server] ➔ [MPSC Funnel] ➔ [WAL fsync] ➔ [Arrow MemTable]
                                                                    │
                                                  ┌─────────────────┴─────────────────┐
                                                  ▼                                   ▼
                                           [SSE Live Stream]                [DataFusion Queries]
                                                  │                                   │
                                           [Vite Dashboard] ◄─────────────────────────┘
```

- **Zero-Loss Write Ahead Log (WAL):** Incoming logs are buffered by the SDK and flushed to Greplog. The engine performs an fsync to `current.wal` before acknowledging HTTP 200 OK back to the SDK.
- **Arrow MemTable:** Committed logs immediately enter an in-memory Apache Arrow `RecordBatch` for instant querying and live-tailing over Server-Sent Events (SSE).
- **Dual-Tier Parquet Storage:**
  - **Real-time Flusher:** Flushes memory to Parquet chunks on a row-count threshold (10,000 rows) or a periodic interval (10 seconds), whichever comes first.
  - **Background Compactor:** Merges crowded partitions into a single highly compressed chunk every hour.
- **Auto Retention (TTL):** Automatically purges Parquet directories older than your specified `--retention-days` (default: 30 days) without heavy SQL DELETE queries.

## CLI Commands

```bash
# Start local dev instance (HTTP server + dashboard + engine)
greplog dev

# Start server on a custom port with 14-day log retention
greplog start --port 8080 --retention-days 14

# Check system status and storage usage
greplog status
```

## Configuration Environment Variables

If you prefer not to hardcode parameters inside `greplog.init()`, set these in your app's environment:

| Environment Variable | Description | Example |
|----------------------|-------------|---------|
| `GREPLOG_URL` | Greplog backend server URL | `http://localhost:3000` |
| `GREPLOG_SERVICE_NAME` | Name of your microservice/app | `api-gateway` |
| `GREPLOG_ENV` | Deployment environment | `production` / `dev` |

## License

Apache License 2.0 © Greplog