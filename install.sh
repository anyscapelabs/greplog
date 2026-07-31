#!/bin/sh
#
# Greplog CLI installer.
#
# Downloads the precompiled `greplog` binary for the current platform from a
# GitHub Release, verifies its sha256 against the checksums.txt published
# alongside the release binaries, and installs it into ~/.greplog/bin, adding
# that directory to PATH in the user's shell profile.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/greplog/greplog/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/greplog/greplog/main/install.sh | sh -s -- --version 0.1.0
#   GREPLOG_VERSION=0.1.0 curl -fsSL https://raw.githubusercontent.com/greplog/greplog/main/install.sh | sh
#
# Platform detection and release asset names MUST stay in lock-step with
# .github/workflows/release.yml and docs/distribution/precompiled-binaries.md
# (see ADR-0010) — this script, the release job, and the doc describe the same
# target triples and the same asset names. Do not change one without the
# others.

set -e

repo="greplog/greplog"
base_url="https://github.com/${repo}/releases/download"

err() {
  echo "install.sh: $*" >&2
}

: "${HOME:?install.sh requires \$HOME to be set}"

if ! command -v curl >/dev/null 2>&1; then
  err "curl is required but was not found on PATH"
  exit 1
fi

# --- Version resolution: --version flag or GREPLOG_VERSION env var, else the
# latest release tag from the GitHub API. ----------------------------
version="${GREPLOG_VERSION:-}"
if [ "${1:-}" = "--version" ]; then
  shift
  if [ $# -lt 1 ]; then
    err "--version requires an argument (e.g. --version 0.1.0)"
    exit 1
  fi
  version="$1"
  shift
fi
if [ $# -gt 0 ]; then
  err "unexpected argument '$1' (usage: install.sh [--version TAG])"
  exit 1
fi

if [ -n "$version" ]; then
  case "$version" in
    v*) ;;
    *) version="v${version}" ;;
  esac
else
  json="$(curl -fsSL "https://api.github.com/repos/${repo}/releases/latest" 2>&1)" || {
    err "failed to query the GitHub API for the latest release (check network access)"
    exit 1
  }
  version="$(printf '%s\n' "$json" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
  if [ -z "$version" ]; then
    err "could not determine the latest release tag from the GitHub API response"
    exit 1
  fi
fi

# --- Platform detection. This must match the release targets in
# release.yml exactly. ------------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux) os="linux" ;;
  Darwin) os="darwin" ;;
  *)
    err "unsupported operating system: $os (expected Linux or Darwin; Windows is supported via WSL2, see ADR-0002)"
    exit 1
    ;;
esac
case "$arch" in
  x86_64|amd64) arch="x86_64" ;;
  aarch64|arm64) arch="aarch64" ;;
  *)
    err "unsupported architecture: $arch (expected x86_64 or aarch64)"
    exit 1
    ;;
esac
case "$os" in
  linux) triple="${arch}-unknown-linux-gnu" ;;
  darwin) triple="${arch}-apple-darwin" ;;
esac

asset="greplog-${triple}"
download_url="${base_url}/${version}/${asset}"

echo "Greplog installer: fetching ${asset} (${version})"

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/greplog.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

# --- Download binary + checksums. ------------------------------------
curl -fsSL -o "$tmp_dir/$asset" "$download_url" || {
  err "failed to download ${download_url}"
  exit 1
}
curl -fsSL -o "$tmp_dir/checksums.txt" "${base_url}/${version}/checksums.txt" || {
  err "failed to download checksums from ${base_url}/${version}/checksums.txt"
  exit 1
}

# --- Verify sha256 against the published checksums.txt. --------------
expected="$(grep -E "^[0-9a-f]{64}  ${asset}$" "$tmp_dir/checksums.txt" | awk '{print $1}' | head -n 1)"
if [ -z "$expected" ]; then
  err "no checksum found for ${asset} in checksums.txt; refusing to install"
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp_dir/$asset" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$tmp_dir/$asset" | awk '{print $1}')"
else
  err "no sha256 utility found (expected sha256sum or shasum)"
  exit 1
fi

if [ "$actual" != "$expected" ]; then
  err "checksum verification FAILED for ${asset}:"
  err "  expected: ${expected}"
  err "  actual:   ${actual}"
  err "refusing to install a corrupted or tampered binary"
  exit 1
fi
echo "Checksum verified (${actual})"

# --- Install to ~/.greplog/bin. --------------------------------------
bin_dir="$HOME/.greplog/bin"
mkdir -p "$bin_dir"
install -m 0755 "$tmp_dir/$asset" "$bin_dir/greplog"
echo "Installed greplog ${version} to ${bin_dir}/greplog"

# --- Add to PATH in the user's shell profile. ------------------------
path_line='export PATH="$HOME/.greplog/bin:$PATH"'
case "$SHELL" in
  */zsh) profile="$HOME/.zshrc" ;;
  */bash) profile="$HOME/.bashrc" ;;
  *) profile="$HOME/.profile" ;;
esac

case ":$PATH:" in
  *":$HOME/.greplog/bin:"*) already_on_path=yes ;;
  *) already_on_path=no ;;
esac

if [ "$already_on_path" = yes ]; then
  echo "${bin_dir} is already on your PATH."
elif [ -f "$profile" ] && grep -Fqs "$path_line" "$profile"; then
  echo "${bin_dir} is already on your PATH via ${profile}."
elif printf '%s\n' "$path_line" >> "$profile" 2>/dev/null; then
  echo "Added ${bin_dir} to your PATH in ${profile}."
  echo "Run 'source ${profile}' (or open a new shell) to use 'greplog'."
else
  echo "Could not write PATH update to ${profile}; add the following line manually:"
  echo "  ${path_line}"
fi

echo "Done. Run 'greplog dev' in your project to start the agent."
