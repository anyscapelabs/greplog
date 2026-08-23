#!/usr/bin/env bash
# Fails when the four SDK manifests disagree on the release version.
set -euo pipefail
cd "$(dirname "$0")/.."

version_from() {
    local manifest=$1
    case "$manifest" in
        *.json) sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$manifest" | head -1 ;;
        *.toml) sed -n 's/^version = "\([^"]*\)".*/\1/p' "$manifest" | head -1 ;;
    esac
}

node=$(version_from sdk/node/package.json)
python=$(version_from sdk/python/pyproject.toml)
rust_sdk=$(version_from sdk/rust/Cargo.toml)
workspace=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)

echo "node=$node python=$python rust_sdk=$rust_sdk workspace=${workspace:-unset}" >&2

if [ "$node" != "$python" ] || [ "$node" != "$rust_sdk" ]; then
    echo "ERROR: SDK versions drifted (node=$node python=$python rust=$rust_sdk)" >&2
    exit 1
fi

if [ -n "${workspace:-}" ] && [ "$workspace" != "$node" ]; then
    echo "ERROR: engine workspace ($workspace) and SDKs ($node) diverged" >&2
    exit 1
fi

echo "all versions agree: $node"
