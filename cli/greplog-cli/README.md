# greplog-cli

CLI wrapper over the greplog agent (`greplog dev`, `greplog status`).

## What this is

Thin CLI binary that starts the agent in foreground or background mode, provides `greplog status` for agent health/resources/query info, and `greplog dev` for local development. Packages the `greplog-agent` as a library dependency. Also ships a `bench_throughput` binary for ingest performance benchmarking.

## Building

```sh
cargo build -p greplog-cli
```

## Testing

```sh
cargo test -p greplog-cli
```

## Structure

```
src/
├── main.rs              — binary entry point ("greplog" binary)
├── bin/
│   └── bench_throughput.rs — ingest throughput benchmark ("bench_throughput" binary)
├── commands/            — mod.rs (banner, subcommand dispatch)
```

## Relationship to the rest of greplog

The CLI is the primary user-facing entry point. It depends on `greplog-agent` and `greplog-core`. See [docs/architecture/cli.md](/docs/architecture/cli.md).
