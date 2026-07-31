# Quickstart

Get Greplog running on your machine in under 60 seconds.

## Prerequisites

- **macOS, Linux, or WSL2** (Windows native pending — see ADR-0002)
- **`curl`** (present on macOS/Linux by default)

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/greplog/greplog/main/install.sh | sh
```

The installer downloads the precompiled `greplog` CLI binary for your platform into `~/.greplog/bin/` (verifying its sha256 against the release checksums) and puts it on your `PATH`. See [distribution/precompiled-binaries.md](distribution/precompiled-binaries.md) for details.

## Start

```bash
cd your-project
greplog dev
```

This starts the local agent and prints the dashboard URL. Open `http://localhost:4317` in your browser.

## Send data

### Node.js

```bash
npm install greplog
```

```typescript
import { greplog } from 'greplog';

greplog.init({ service: 'my-service' });
greplog.info('server started', { port: '8080' });
```

### Python

```bash
pip install greplog
```

```python
import greplog

greplog.init(service='my-service')
greplog.info('server started', details={'port': '8080'})
```

### Go

```bash
go get github.com/greplog/greplog-go
```

```go
import greplog "github.com/greplog/greplog-go/src"

greplog.Init(&greplog.Options{ServiceName: "my-service"})
greplog.Info("server started")
```

### Rust

```bash
cargo add greplog
```

```rust
greplog::init();
greplog::info!("server started");
```

## See data

Open `http://localhost:4317` in your browser. Events appear in the timeline within seconds.

## Next steps

- [`greplog init`](architecture/cli.md) — detect framework and configure automatically
- [`greplog status`](architecture/cli.md) — check agent health
- [Agent pipeline](architecture/agent-pipeline.md) — understand how data flows
