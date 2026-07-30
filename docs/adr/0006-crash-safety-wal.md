# ADR-0006: Crash Safety — Write-Ahead Log

**Status:** Accepted
**Date:** 2026-03-01

## Context

If the agent crashes between receiving an ingest batch and persisting it to Parquet, that data is lost. The earlier approach relied on the filesystem write cache, which is insufficient for crash safety guarantees.

## Decision

Write every incoming batch to a write-ahead log (WAL) on disk before sending the acknowledgment. On recovery, replay the WAL through dedup → buffer → flush.

## Alternatives considered

- **Rely on filesystem write cache:** Unsafe — power loss or kernel panic before `fsync` causes data loss.
- **Double-write to Parquet immediately:** Too slow for high-throughput ingest (Parquet encoding is expensive per batch).

## Consequences

- Zero data loss on crash: WAL replay guarantees every acknowledged batch is eventually persisted.
- WAL segments are truncated after successful flush — bounded disk usage.
- Recovery adds startup latency proportional to WAL size (typically <1s for normal operation).
