# SDK Design Principles

Each SDK is intentionally thin and uses Protobuf as the universal wire format. Business logic belongs in the agent.

## Capture Surface

- HTTP request/response (status, latency, route)
- Uncaught exceptions
- Unhandled promise rejections / panics
- Structured log calls

## Pseudo-Tracing (v1)

To keep SDKs bulletproof and avoid breaking user HTTP clients, v1 does not auto-inject W3C `traceparent` headers. Instead, it relies on perfect local system clock synchronization to align logs, passing optional manual `correlation_id` fields if the user provides them.

## Transport

A lightweight async client that connects to the workspace socket. If UDS fails, it falls back to TCP.

## Fail-Open with One Warning

If the agent isn't running at all, the SDK emits exactly one warning to stdout (`[Greplog] Agent not found. Run 'greplog dev' to capture logs.`) on startup. After that, it fails silently and drops events. It must never crash the host app.

## Redaction Hooks

SDKs accept a config for scrubbing fields before events leave the process, in addition to agent-side redaction.
