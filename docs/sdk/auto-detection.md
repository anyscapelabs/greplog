# SDK Self-Detection

> **Status:** ✅ Shipped (v0.1)

This document covers **SDK self-detection** — what happens inside an SDK's `init()` at runtime. This is different from the **agent's workspace detection** (the `/detect` endpoint scanning `package.json`/`Cargo.toml`/`go.mod` from outside any process). See [`../architecture/agent-pipeline.md`](../architecture/agent-pipeline.md) for workspace detection.

## How It Works

When `greplog.Init()` is called in any language SDK, it:

1. **Reads the project file** (`package.json`, `Cargo.toml`, `go.mod`, `pyproject.toml`) from the current working directory.
2. **Extracts the service name** from the `name` field.
3. **Detects the web framework** by scanning dependencies:
   - Node: Express, Fastify, NestJS, Next.js
   - Python: Flask, Django, FastAPI
   - Go: Gin, Echo, Fiber, Chi, Gorilla Mux
   - Rust: Axum, Actix Web, Tower
4. **Writes `greplog.config.json`** with detected service name and framework.
5. **Returns `detectedFramework`** for the SDK to auto-register middleware.

## Detection by Language

| Language | Project File | Frameworks Detected |
|---|---|---|
| Node.js | `package.json` | Express, Fastify, NestJS, Next.js |
| Python | `pyproject.toml` | Flask, Django, FastAPI |
| Go | `go.mod` | Gin, Echo, Fiber, Chi, Gorilla Mux |
| Rust | `Cargo.toml` | Axum, Actix Web, Tower |

## Design Notes

- **Fail-open:** If no project file is found, the SDK proceeds with the caller-provided service name or a hostname default.
- **Cached:** Service name and framework are detected once per process lifetime (cached after first `init()`).
