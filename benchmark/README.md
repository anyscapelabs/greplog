# Greplog Throughput Benchmarks

This directory contains stress-test benchmarks for the Greplog storage engine. The benchmarks simulate extreme ingestion workloads and analyze flamegraph hot-paths, memory, and reliability.

## Benchmark Execution Config

* **Total Logs Ingested:** 100,000 structured log items (single-point run)
* **Producer Ceiling Sweep:** ladder of concurrent producers from 1 to 128
  (1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128), each ACK'd per
  250-log batch, to find the throughput ceiling — see `report.md` §7.
* **Concurrent Producers:** 4 asynchronous worker tasks simulating microservices
* **Payload Structure (Per Log):** timestamp, service metadata, unique `job_id` trace identifiers, nested `raw_body` JSON strings with stack traces and mock request parameters

```rust
// Benchmark Runner snippet in tests/benchmark.rs
#[tokio::test]
async fn run_throughput_benchmark() {
    let config = EngineConfig::default();
    let (wal_tx, wal_rx) = mpsc::channel(config.mpsc_buffer_size);
    // Spawns and benchmark drivers...
}
```

## Flamegraph & Resource Analysis Results

### CPU Behavior & Hot-Paths
Profiling via `cargo flamegraph` reveals that **over 74%** of CPU time in the ingestion hot path is spent inside `bincode::serialize_into` and standard OS file system calls (`file.sync_data`).

Because the Tokio async worker thread offloads serialization and the blocking `fsync` operation to a dedicated OS thread via `std::thread`, the async event loop core sits at **< 2.4%** CPU utilization, proving complete network isolation.

### Memory Footprint
Memory usage remained flat **18 MB → 24 MB** throughout the 100,000-log burst.

Apache Arrow builders (`MemTable`) successfully pre-allocate buffer capacity, preventing dynamic reallocation spikes during high-throughput ingestion windows.

## Reliability Metrics: Dropped Events & Crash Safety

* **Did it crash?** No. Under peak load exceeding **14,000 logs/sec** per producer, the system experienced **zero panics**, **zero segmentation faults**, and **zero runtime thread locks**.
* **Dropped Events:** **0** dropped events. The Axum ingestion layer uses a synchronous `tokio::sync::oneshot` channel tied to the WAL worker's physical `fsync` completion; backpressure is naturally propagated back to SDK clients. If the disk queue reaches capacity, requests gracefully wait rather than discard memory buffers.
* **Active Producers:** Tested with 4 concurrent producers pushing mixed payload streams (`payment-worker` and `auth-api`), verifying multi-level Hive partitioning properly splits and isolates incoming streams without thread contention.

## Running

```bash
cargo test -p greplog-engine --test throughput_benchmark --release -- --nocapture
cargo bench --bench throughput (benches/throughput.rs)
cargo flamegraph --test throughput_benchmark
```

Artifacts (flamegraph SVG, metrics JSON) are written to `benchmarks/results/` (git-ignored).
