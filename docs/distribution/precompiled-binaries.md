# Precompiled Binary Distribution

> **Status:** ✅ Shipped (v0.1) — macOS/Linux; Windows via WSL2 only

The `greplog-cli` npm package ships as a thin Node wrapper. Its only job is to detect `process.platform` + `process.arch`, and download the matching precompiled agent binary from a GitHub Release into `~/.greplog/bin/`. Subsequent invocations exec the cached binary.

For v1, Windows support relies on WSL2 (which fully supports Unix sockets); native Windows named pipes are deferred to accelerate time-to-market.
