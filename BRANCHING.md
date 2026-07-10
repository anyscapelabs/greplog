# Branching Strategy

## Branches

| Branch      | Purpose                                                    |
|-------------|------------------------------------------------------------|
| `main`      | Production-ready code. All merges require review.          |
| `staging`   | Pre-production integration. Features merged here for QA.   |
| `feature/*` | Individual feature or task branches.                       |

## Workflow

1. Create a `feature/<name>` branch off `staging`.
2. Develop and commit on the feature branch.
3. Open a pull request into `staging`.
4. After review and testing, merge into `staging`.
5. When `staging` is stable, merge `staging` into `main`.

## Active Feature Branches

| Branch                             | Status     | Description                      |
|------------------------------------|------------|----------------------------------|
| `feature/project-foundation`       | In progress | Docs, LICENSE, README, structure |
| `feature/rust-workspace-core`      | Pending    | `greplog-core` with Protobuf schema |
| `feature/agent-ingest`             | Pending    | UDS/TCP ingest, DuckDB writer    |
| `feature/sdk-node`                 | Pending    | Node.js SDK with auto-detection  |
| `feature/agent-query-api`          | Pending    | `POST /query` SQL translation    |
| `feature/dashboard-embed`          | Pending    | React dashboard embedded in agent |
| `feature/cli`                      | Pending    | `greplog dev`, `init`, `status`  |
| `feature/ci-release`               | Pending    | GitHub Actions, binary releases  |

## Merging

- `feature/*` → `staging` — squash merge
- `staging` → `main` — merge commit
