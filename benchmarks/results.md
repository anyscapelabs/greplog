# Official benchmarks

Numbers from [`.github/workflows/benchmarks.yml`](../.github/workflows/benchmarks.yml)
on GitHub-hosted `ubuntu-latest` (x86_64, 4 vCPU, 16 GB RAM), exercising the
real production path — WAL fsync-before-ack ingestion and DataFusion queries
over flushed Parquet (`crates/engine/tests/e2e_benchmark.rs`). One section per
run, newest last. The producer-ladder sweep in `report.md` uses a mock WAL to
find the ceiling of the producer pipeline; those numbers are not comparable.
