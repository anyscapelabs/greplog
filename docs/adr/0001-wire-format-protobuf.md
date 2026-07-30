# ADR-0001: Wire Format — Protobuf

**Status:** Accepted
**Date:** 2026-03-01

## Context

SDKs in four languages (Go, Python, Node, Rust) need to communicate with the agent over UDS and TCP. The wire format must support strict schema evolution and be available natively in all four languages.

## Decision

Use Protocol Buffers (protobuf) for all ingest and response messages.

## Alternatives considered

- **FlatBuffers / Cap'n Proto:** Less ecosystem support across all four target languages.
- **MessagePack:** No built-in schema evolution; would require a custom versioning layer.
- **JSON:** Too large for high-throughput ingest; parsing overhead at 10k+ ev/s.

## Consequences

- All four languages can generate code from the same `.proto` file.
- Prost crate compiles protos to native Rust structs at build time — no runtime proto parser in the agent.
- Adds a build step (code generation) for each SDK.
