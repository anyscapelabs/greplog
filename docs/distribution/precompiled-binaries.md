# Precompiled Binary Distribution

> **Status:** ✅ Shipped — macOS/Linux via `curl | sh`; Windows via WSL2 only

The CLI is installed via a shell-script installer, not an npm package:

```bash
curl -sSL https://raw.githubusercontent.com/greplog/greplog/main/install.sh | sh
```

The installer detects `uname` (platform + arch), downloads the matching
precompiled agent binary from a GitHub Release into `~/.greplog/bin/`, and
adds it to `PATH` (or prints the path to add if the shell config can't be
written). Subsequent invocations exec the cached binary; re-running the
installer updates it.

For v1, Windows support relies on WSL2 (which fully supports Unix sockets); native Windows named pipes are deferred to accelerate time-to-market.
