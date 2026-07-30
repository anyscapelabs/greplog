# ADR-0004: Distributed Tracing — Deferred

**Status:** Accepted
**Date:** 2026-03-01

## Context

Full W3C trace-context propagation across services requires every SDK to implement propagators for HTTP/gRPC, plus a trace-tree reconstruction query layer. This is significant work across four SDKs and the query engine.

## Decision

Defer distributed tracing beyond v0.1. v1 relies on chronological interleaving (pseudo-tracing across a shared timeline) plus optional manual `correlation_id`.

## Alternatives considered

- **Ship W3C traceparent in v0.1:** Would delay v0.1 by 6-8 weeks for SDK + query engine work across all four languages.
- **Skip tracing entirely:** Too limiting — chronological interleaving is the minimum viable version.

## Consequences

- SDKs stay thin and stable — no trace propagation logic in v0.1.
- Dashboard users see interleaved logs from all services in time order, but no formal trace waterfall view.
- W3C trace propagation will come in a later phase as a backward-compatible addition.
