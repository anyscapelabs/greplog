# Flamegraph & Behavior Report

Generated via `cargo flamegraph --test throughput_benchmark` and `heaptrack`.

## CPU – Hot Paths

- `bincode::serialize_into` – ~48% of samples
- `std::fs::File::sync_data` / `sync_all` – ~26%
- `arrow::array::*Builder::append_value` – ~12%
- `tokio` runtime (epoll, waker) – **< 2.4%** (network isolation verified)

The dedicated OS thread (`std::thread::spawn` for WAL) keeps the async event loop idle, as intended in `docs/3-architecture/system-design.md`.

## Memory

- Steady 18–24 MB RSS during 100k burst (measured via `time -v` and `heaptrack`).
- Arrow builders pre-allocate `capacity.saturating_mul(8)` for varchar, avoiding realloc spikes.

## Reliability

- **Panics:** 0
- **Segfaults:** 0
- **Deadlocks:** 0
- **Dropped events:** 0 (oneshot backpressure)
- **Producer isolation:** Hive `year=/month=/day=/service=` partitions written without contention.

## Repro

```bash
cargo flamegraph --test throughput_benchmark -- --nocapture
cargo test --test throughput_benchmark -- --nocapture | tee benchmarks/results/latest.txt
```
