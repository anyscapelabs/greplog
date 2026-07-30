# ADR-0002: Windows Support — v1 Unix-Only

**Status:** Accepted
**Date:** 2026-03-01

## Context

Adding native Windows named-pipe support would roughly double the transport-layer surface area and testing matrix. The v1 priority is macOS and Linux (the primary development OS for the AI-assisted coding target audience).

## Decision

v1 ships Unix-only (macOS, Linux, WSL2 on Windows). Native Windows named pipes are deferred.

## Alternatives considered

- **Full Windows support from day one:** Would delay v1 by an estimated 4-6 weeks for transport, path handling, and testing across Windows CI.
- **WSL2-only:** Accepted as the interim path — most Windows developers already use WSL2.

## Consequences

- Windows users must run via WSL2 for v1.
- Agent install scripts must detect OS and provide a clear WSL2 setup guide.
- Named-pipe support can be a follow-up without breaking wire compatibility.
