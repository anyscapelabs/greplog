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

One command removes everything — storage, WAL and the binary itself:

```bash
greplog uninstall
```

The uninstaller lists what it found with sizes, warns if a Greplog instance is still serving, and asks for confirmation before deleting. For scripts, skip the prompt:

```bash
greplog uninstall --yes
```

Storage resolves to the same location every other command uses — `~/.local/share/greplog` on Linux, `~/Library/Application Support/greplog` on macOS, or `$GREPLOG_DATA_DIR` when set — so the command reports exactly what it sees before touching anything.
