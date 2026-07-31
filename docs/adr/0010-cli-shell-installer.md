# ADR-0010: CLI Distribution — Shell-Script Installer

**Status:** Accepted
**Date:** 2026-07-31

## Context

The CLI was originally planned to be distributed as an npm wrapper package:
a thin Node.js package that detects the user's platform and downloads a
precompiled Rust binary from a GitHub Release into `~/.greplog/bin/`
(described in `docs/distribution/precompiled-binaries.md`).

That wrapper is an unnecessary Node-dependent layer for what is a Rust
binary. Installing a Rust tool via npm means the CLI is only installable where
Node is present, it adds an extra dependency to resolve (`npm i -g`),
and it introduces a second binary-fetching mechanism (npm's lifecycle hooks
plus the platform-detect-and-download logic) that must be kept in sync with
the release pipeline for no benefit. The standard pattern for Rust-binary
CLIs is a `curl | sh` shell-script installer.

Also, v0.1 has not shipped and there is no existing install base, so there is
nothing to migrate. A deprecation window for the npm wrapper would only be
warranted post-release; removing it now is free.

## Decision

The npm-wrapper CLI distribution package is **removed completely** and
replaced by a shell-script installer (`install.sh` at the repo root) as the
**sole** CLI install method:

```sh
curl -fsSL https://raw.githubusercontent.com/greplog/greplog/main/install.sh | sh && greplog dev
```

The installer:
1. Detects OS (Linux/Darwin) and arch (x86_64/aarch64) via `uname`, mapping to
   the same target triples the release pipeline builds.
2. Resolves the version to install from the latest GitHub Release, or honors
   `GREPLOG_VERSION` / `--version` for pinning.
3. Downloads the matching precompiled binary from the GitHub Release produced
   by `.github/workflows/release.yml`.
4. Verifies the binary's sha256 against the `checksums.txt` published
   alongside the release assets — checksum verification is a hard requirement;
   a mismatch fails loudly rather than installing.
5. Installs to `~/.greplog/bin`, makes it executable, and adds it to PATH in
   the user's shell profile (`~/.zshrc`/`~/.bashrc`/`~/.profile`), printing
   instructions if the profile can't be written.

**This ADR SUPERSEDES the npm-wrapper CLI distribution mechanism** described
in `docs/distribution/precompiled-binaries.md`. The release workflow
(`.github/workflows/release.yml`) explicitly has no CLI-wrapper npm publish
job.

## Alternatives considered

- **Keep the npm wrapper as one of two install methods:** More surface area to
  keep consistent (two mechanisms that must produce the same result), and no
  migration cost saved since nothing has shipped.
- **Direct GitHub Release download with no installer:** Pushes platform
  detection and checksum verification onto the user; worse DX than a one-liner.
- **Homebrew as the sole method:** Linux users are unserved; a shell installer
  covers both.

## Consequences

- `docs/distribution/precompiled-binaries.md` is rewritten to describe the
  shell-script installer, the GitHub Release asset naming, and checksum
  verification as the current mechanism.
- The README headline install command changes to the `curl | sh` one-liner;
  the old `npm i -g greplog` CLI-install line is removed.
- The **Node SDK npm package (`sdks/node`) is completely unaffected** — `npm i
  greplog` as a project dependency is a different package than the removed CLI
  wrapper.
- `install.sh` is served from the raw GitHub URL; a future
  `get.greplog.dev` redirect is a documented follow-up (ROADMAP
  Distribution), not implemented now.
- CI/CD (`ci.yml`, `release.yml`) is the enforcement mechanism for the
  AGENTS.md pre-completion checklist, and `release.yml` publishes the release
  binaries the installer consumes.
