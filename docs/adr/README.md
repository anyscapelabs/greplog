# Architecture Decision Records

Numbered, dated records of significant architectural decisions. Each ADR is a short file describing the context, decision, alternatives considered, and consequences.

## Active ADRs

| # | Title | Date | Status |
|---|-------|------|--------|
| 0001 | [Wire Format — Protobuf](0001-wire-format-protobuf.md) | 2026-03-01 | Accepted |
| 0002 | [Windows — v1 Unix-Only](0002-windows-v1-unix-only.md) | 2026-03-01 | Accepted |
| 0003 | [Agent Scope — One Per Workspace](0003-agent-scope-per-workspace.md) | 2026-03-01 | Accepted |
| 0004 | [Distributed Tracing — Deferred](0004-distributed-tracing-deferred.md) | 2026-03-01 | Accepted |
| 0005 | [Storage — Arrow + Parquet](0005-storage-backend-arrow-parquet.md) | 2026-03-01 | Accepted |
| 0006 | [Crash Safety — WAL](0006-crash-safety-wal.md) | 2026-03-01 | Accepted |
| 0007 | [Dedup — Content-Addressed BLAKE3](0007-dedup-content-addressed.md) | 2026-03-01 | Accepted |
| 0008 | [Compaction — Background, Idempotent](0008-compaction-background-idempotent.md) | 2026-03-01 | Accepted |
| 0009 | [SDK Startup — Fail-Open](0009-sdk-startup-fail-open.md) | 2026-03-01 | Accepted |

## Adding a new ADR

1. Copy `NNNN-template.md` (if available) or use the format from an existing ADR.
2. Assign the next sequential number.
3. Set `Status` to one of: `Accepted`, `Superseded by ADR-NNNN`, `Deprecated`.
4. Link it from this README's table.
