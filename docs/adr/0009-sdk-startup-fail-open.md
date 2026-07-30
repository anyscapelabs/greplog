# ADR-0009: SDK Startup — Fail-Open

**Status:** Accepted
**Date:** 2026-03-01

## Context

The SDK must connect to the local agent at startup. If the agent isn't running (e.g., user forgot `greplog dev`, or agent crashed), the SDK should not crash the application.

## Decision

SDK starts in fail-open mode: log exactly one warning (`Agent not found`), then silently drop all subsequent events until the agent becomes available.

## Alternatives considered

- **Block until agent available:** Blocks application startup; poor UX.
- **Panic/crash on missing agent:** Unacceptable for a dev-tool dependency.
- **Aggressive reconnect:** Wastes CPU on polling if agent is intentionally not running.

## Consequences

- Application starts and runs normally even without the agent.
- One visible warning per process lifetime tells the user what's missing.
- Events are dropped while the agent is down — acceptable for local dev; production deployments should ensure the agent is running.
- Periodic reconnection attempts (configurable interval) re-establish the connection when the agent starts.
