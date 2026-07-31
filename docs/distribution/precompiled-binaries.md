# Precompiled Binary Distribution

> **Status:** ✅ Shipped — shell-script installer; macOS/Linux via `curl | sh`; Windows via WSL2 only

The CLI is distributed as a precompiled Rust binary installed via a
shell-script installer, **not** an npm package (see
[ADR-0010](../adr/0010-cli-shell-installer.md) — the earlier npm-wrapper CLI
distribution mechanism is superseded and removed).

## One-liner

```bash
curl -fsSL https://raw.githubusercontent.com/greplog/greplog/main/install.sh | sh && greplog dev
```

`-f` is deliberate: a failed HTTP download must fail loudly instead of
piping an error page into `sh`.

## What the installer does

1. Detects the OS and architecture via `uname` and maps them to a target
   triple (see the asset table below).
2. Resolves the version: `GREPLOG_VERSION` env var or `--version` flag for
   pinning, otherwise the latest release tag from the GitHub API.
3. Downloads the matching precompiled `greplog` binary from the GitHub
   Release.
4. **Verifies the binary's sha256 against the `checksums.txt` published
   alongside the release assets** — a mismatch aborts the install with an
   error (checksum verification is a hard requirement; there is no silent
   fallback).
5. Installs the binary to `~/.greplog/bin/`, makes it executable, and adds
   that directory to `PATH` in `~/.zshrc`, `~/.bashrc`, or `~/.profile`
   (whichever matches the user's shell), or prints the exact line to add if
   the profile can't be written.

Re-running the installer updates the binary to the latest version.

## Release assets (naming scheme)

`.github/workflows/release.yml` publishes, per release tag, one binary per
supported target for each of `greplog` (the CLI) and `greplog-agent`, plus a
`checksums.txt`:

| Target | Asset names |
|--------|-------------|
| `x86_64-unknown-linux-gnu` | `greplog-x86_64-unknown-linux-gnu`, `greplog-agent-x86_64-unknown-linux-gnu` |
| `aarch64-unknown-linux-gnu` | `greplog-aarch64-unknown-linux-gnu`, `greplog-agent-aarch64-unknown-linux-gnu` |
| `x86_64-apple-darwin` | `greplog-x86_64-apple-darwin`, `greplog-agent-x86_64-apple-darwin` |
| `aarch64-apple-darwin` | `greplog-aarch64-apple-darwin`, `greplog-agent-aarch64-apple-darwin` |

`checksums.txt` contains the `sha256` of every asset in the release
(`<hash>  <asset-name>` lines, `sha256sum` format). The installer downloads
the `greplog-<target>` asset for the user's platform and verifies it against
this file.

**`install.sh`, `release.yml`, and this document MUST agree on these asset
names and the platform-to-triple mapping.** They are the same values — never
change one without the other.

## Version pinning

```bash
curl -fsSL https://raw.githubusercontent.com/greplog/greplog/main/install.sh | sh -s -- --version 0.1.0
# or
GREPLOG_VERSION=0.1.0 curl -fsSL https://raw.githubusercontent.com/greplog/greplog/main/install.sh | sh
```

Versions are unified `v*` tags (e.g. `v0.1.3`) versioning the agent, CLI, and
all SDKs together (monorepo decision).

## Windows

For v1, Windows support relies on WSL2 (which fully supports Unix sockets);
native Windows named pipes are deferred (ADR-0002). On WSL2 the installer runs
as-is.

## Follow-ups

- `get.greplog.dev` redirect for the installer one-liner — tracked in
  ROADMAP Distribution; install.sh is served from the raw GitHub URL until a
  domain is available.
