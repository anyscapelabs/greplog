# Local Development

Greplog is a Rust Cargo workspace with an embedded Vite frontend.

## Prerequisites

- Rust stable (`rustup`)
- Node.js 18+ and a package manager (`npm` / `pnpm`)
- Cargo build dependencies for `parquet` and `datafusion` (protobuf, zstd, etc.)

## Clone and build

```bash
git clone https://github.com/anyscapelabs/greplog.git
cd greplog
cargo build
```

## Run the dev instance

```bash
cargo run -- dev
```

The Vite dashboard is built and embedded into the binary for `dev`. For frontend iteration you can run Vite in dev mode against the backend:

```bash
cd web
npm install
npm run dev
```

## Workspace layout

```
greplog/
├── crates/
│   ├── greplog-server/   # Axum server, WAL, storage engine
│   └── greplog-sdk/      # reference Rust SDK (mp/sc batching)
├── web/                  # Vite + embedding for the dashboard
├── docs/                 # these docs
└── assets/               # branding and logo assets
```

## Tests

```bash
cargo test --workspace
```

Run the SDK integration test against a live server on `localhost:3000` for a full end-to-end check.

## Useful commands

```bash
cargo run -- status   # storage and health
cargo run -- start --port 8080 --retention-days 7
```

See `CONTRIBUTING` conventions in [roadmap.md](roadmap.md) and keep the [Code of Conduct](code-of-conduct.md) in mind.