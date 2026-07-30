# Benchmarks

Manual or scheduled performance measurements. These are **not** CI regression
gates — they are environment-sensitive and produce noisy numbers that should
not block PRs. Use them for capacity planning, release comparison, and
regression *investigation* (not automated gating).

## Machine assumptions

Numbers are only meaningful relative to the hardware/OS they were collected on:

- CPU cores / architecture
- Disk type (NVMe, SSD, HDD) and filesystem
- RAM size and speed
- Kernel version and IO scheduler

Record this context alongside every baseline you pin.

## Benchmark suites

- `ingest_throughput/` — Agent ingest ceiling (Round 9–11 producer-ceiling benchmarks).
- `query_latency/` — Query latency/throughput (Round 12+).

## Baselines

Pinned v0.1 baseline numbers belong in `BASELINE.md` at this directory's root.
Before committing a new baseline, record the full machine config above.
