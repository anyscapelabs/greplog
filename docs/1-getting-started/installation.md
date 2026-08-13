# Installation

Install the `greplog` CLI with a single command:

```bash
curl -fsSL https://greplog.dev/install.sh | sh
```

The installer downloads the latest compiled binary for your platform, verifies checksums, and places `greplog` on your `PATH`.

## Verify the install

```bash
greplog --version
```

## Basic setup

Start a local dev instance:

```bash
greplog dev
```

This spins up:

- **Ingest Server** at `http://localhost:5050/api/log`
- **Embedded Vite Dashboard** at `http://localhost:3000`
- **DataFusion Query Engine** over local Parquet storage

Run in production mode with custom settings:

```bash
greplog start --port 8080 --retention-days 14
```

## Platform support

- Linux (x86_64, aarch64)
- macOS (x86_64, Apple Silicon)
- Windows (via WSL2)

## Uninstall

Remove the binary and its data directory:

```bash
rm -f "$(command -v greplog)" && rm -rf data/logs data/wal
```