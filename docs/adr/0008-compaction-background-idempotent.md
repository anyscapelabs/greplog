# ADR-0008: Compaction — Background, Idempotent

**Status:** Accepted
**Date:** 2026-03-01

## Context

Frequent flushes (every 2 seconds) produce many small Parquet files — hundreds per hour at high throughput. Small files degrade query performance and waste disk space. The compaction process must not block ingest or queries.

## Decision

Background compaction scheduler that merges small Parquet partitions. Idempotent by design — partial merges are discarded on crash.

## Alternatives considered

- **Synchronous compaction:** Blocks the flush pipeline; unacceptable for ingest latency.
- **Compaction on flush only:** Misses cross-partition optimization opportunities.
- **Skip compaction entirely:** Query performance degrades over time as small files accumulate.

## Consequences

- Compaction runs on a configurable interval (default 5 min).
- Skips the current hour's partition to avoid flush contention.
- Uses atomic file replacement (write `.tmp`, then rename) — crash during merge only loses the in-progress output, not the source files.
- Idempotent: each run records its output manifest; interrupted runs leave no partial state.
