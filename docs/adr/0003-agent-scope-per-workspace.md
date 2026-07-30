# ADR-0003: Agent Scope — One Agent Per Workspace

**Status:** Accepted
**Date:** 2026-03-01

## Context

Monorepos have multiple services that all need to send observability data. A single global agent is simpler but conflates data from unrelated projects. A per-service agent over-complicates monorepo workflows.

## Decision

One agent per workspace (project root), binding to a `.greplog/greplog.sock` UDS socket and a local TCP fallback port. Each workspace gets an isolated agent with its own data directory.

## Alternatives considered

- **Global single agent:** Risk of data mixing between projects; harder to clean up.
- **Per-service agent:** Too many processes; SDK would need to discover which agent to talk to.

## Consequences

- Monorepo services all stream to the same local socket — multiplexed naturally.
- Opening a second project spawns an isolated agent on a different port.
- Data lives under `~/.greplog/<project-hash>/`.
