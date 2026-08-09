# crates

The Greplog Rust workspace crates.

| Crate | Path | Purpose |
|-------|------|---------|
| `greplog-engine` | `crates/engine/` | Typed log engine: error domain, `LogRecord`, Arrow schema, WAL | 
| `greplog-server` | `crates/server/` | Axum ingest + dashboard server (Round 3+) |
| `greplog-cli` | `crates/cli/` | `greplog dev` / `start` / `status` CLI (Round 4+) |

See [crates/engine/README.md](engine/README.md) for the engine crate.