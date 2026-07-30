# ADR-0007: Dedup — Content-Addressed (BLAKE3)

**Status:** Accepted
**Date:** 2026-03-01

## Context

SDKs retry on network errors, producing duplicate events. The agent must detect and drop duplicates. SDK-side dedup would require each SDK to maintain state, adding complexity to all four language implementations.

## Decision

Content-addressed dedup at the agent level using BLAKE3 hashes of serialized event bytes. The agent is the single source of truth for dedup decisions.

## Alternatives considered

- **SDK-side dedup with persistent IDs:** Requires each SDK to maintain state across restarts; four implementations to keep in sync.
- **Dedupe by event_id / ULID:** Works only if SDKs always provide unique IDs; not guaranteed for all use cases.

## Consequences

- SDKs remain stateless — they can blindly retry and let the agent deduplicate.
- BLAKE3 is fast enough to hash at line rate (microseconds per event).
- The dedup index is persisted to disk (`dedup.idx`) so it survives agent restarts.
- Memory usage scales with unique event count, not total event count — most practical workloads see high duplicate rates from retries.
