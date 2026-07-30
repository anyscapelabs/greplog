#!/usr/bin/env bash
set -euo pipefail

RELEASE_DIR="/home/brnx/Desktop/greplog/target/release"
AGENT_BIN="${RELEASE_DIR}/greplog-agent"
BENCH_BIN="${RELEASE_DIR}/bench_throughput"
RESULTS_DIR="/home/brnx/Desktop/greplog/throughput_results"
AGENT_LOG="/tmp/greplog_throughput_agent.log"

mkdir -p "$RESULTS_DIR"

run_bench() {
    local producers=$1
    local tag=$2
    local workspace="/tmp/greplog_throughput_${tag}"
    local socket="${workspace}/.greplog/greplog.sock"

    echo ""
    echo "======================================================================"
    echo "  PRODUCERS=${producers}  tag=${tag}"
    echo "======================================================================"
    echo ""

    # Clean and create workspace
    rm -rf "$workspace"
    mkdir -p "${workspace}/.greplog"

    # Start agent
    RUST_LOG=info "$AGENT_BIN" \
        --workspace "$workspace" \
        --tcp-port 0 \
        --ui-port 0 \
        --socket-path ".greplog/greplog.sock" \
        --flush-interval-secs 2 \
        --max-buffer-events 50000 \
        > "$AGENT_LOG" 2>&1 &
    local agent_pid=$!
    echo "Agent PID: $agent_pid"

    # Wait for UDS socket to appear
    for i in $(seq 1 30); do
        if [ -S "$socket" ]; then
            echo "Agent ready after ${i}s"
            break
        fi
        sleep 1
    done
    if [ ! -S "$socket" ]; then
        echo "FAIL: Agent UDS socket not found after 30s"
        kill "$agent_pid" 2>/dev/null || true
        return 1
    fi

    # Run throughput benchmark
    # start_rate and rate_step: adjust per producer count to explore the ceiling
    # With more producers, each sends at a lower rate to avoid flooding
    sleep 2  # let agent stabilize

    echo "Running throughput benchmark with ${producers} producer(s)..."
    "$BENCH_BIN" \
        --socket "$socket" \
        --status-host "127.0.0.1:0" \
        --start-rate 100 \
        --rate-step 200 \
        --step-secs 30 \
        --steps 5 \
        --events-per-batch 10 \
        --producers "$producers" \
        --service "bench-${tag}" \
        2>&1 | tee "${RESULTS_DIR}/throughput_${tag}.txt" || true

    # Kill agent
    kill "$agent_pid" 2>/dev/null || true
    wait "$agent_pid" 2>/dev/null || true
    echo "Agent stopped."

    # Small delay between runs
    sleep 2
}

# ── Run scaling curve ──
for producers in 1 4 8 16 32; do
    run_bench $producers "p${producers}"
done

echo ""
echo "=== ALL BENCHMARKS COMPLETE ==="
echo "Results in: ${RESULTS_DIR}"
