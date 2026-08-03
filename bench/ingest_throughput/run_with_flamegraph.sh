#!/usr/bin/env bash
set -euo pipefail

RELEASE_DIR="/home/brnx/Desktop/greplog/target/release"
AGENT_BIN="${RELEASE_DIR}/greplog-agent"
BENCH_BIN="${RELEASE_DIR}/bench_throughput"
RESULTS_DIR="/home/brnx/Desktop/greplog/throughput_results"
AGENT_LOG="/tmp/greplog_throughput_agent.log"

mkdir -p "$RESULTS_DIR"

run_bench_with_profiling() {
    local producers=$1
    local tag=$2
    local workspace="/tmp/greplog_throughput_${tag}"
    local socket="${workspace}/.greplog/greplog.sock"

    echo ""
    echo "======================================================================"
    echo "  PRODUCERS=${producers}  tag=${tag} WITH FLAMEGRAPH"
    echo "======================================================================"
    echo ""

    # Clean and create workspace
    rm -rf "$workspace"
    mkdir -p "${workspace}/.greplog"

    # Start agent with perf profiling via flamegraph
    echo "Starting agent with flamegraph profiling..."
    
    # Run agent in background and capture PID
    cd /home/brnx/Desktop/greplog
    sudo -E env "PATH=$PATH" \
        flamegraph -o "${RESULTS_DIR}/flamegraph_${tag}.svg" \
        --freq 997 \
        -- \
        "$AGENT_BIN" \
        --workspace "$workspace" \
        --tcp-port 0 \
        --ui-port 0 \
        --socket-path ".greplog/greplog.sock" \
        --flush-interval-secs 2 \
        --max-buffer-events 500000 \
        > "$AGENT_LOG" 2>&1 &
    
    local agent_pid=$!
    echo "Agent PID: $agent_pid (with flamegraph profiling)"

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
        sudo kill "$agent_pid" 2>/dev/null || true
        return 1
    fi

    sleep 2  # let agent stabilize

    echo "Running throughput benchmark with ${producers} producer(s)..."
    "$BENCH_BIN" \
        --socket "$socket" \
        --status-host "127.0.0.1:0" \
        --start-rate 1000 \
        --rate-step 2000 \
        --step-secs 20 \
        --steps 10 \
        --events-per-batch 10 \
        --producers "$producers" \
        --service "bench-${tag}" \
        2>&1 | tee "${RESULTS_DIR}/throughput_${tag}.txt" || true

    # Kill agent - give it a moment to finish writing
    sleep 2
    sudo kill -INT "$agent_pid" 2>/dev/null || true
    sleep 3
    sudo kill -9 "$agent_pid" 2>/dev/null || true
    wait "$agent_pid" 2>/dev/null || true
    echo "Agent stopped. Flamegraph saved to ${RESULTS_DIR}/flamegraph_${tag}.svg"

    sleep 2
}

# Run with increasing producer counts to find the bottleneck
echo "=== Running benchmark with flamegraph profiling ==="
echo "This will generate CPU flamegraphs showing where time is spent."
echo ""

# Start with single producer to establish baseline
run_bench_with_profiling 1 "p1"

# Then multi-producer to stress test
run_bench_with_profiling 32 "p32"

echo ""
echo "=== BENCHMARKS COMPLETE ==="
echo "Results in: ${RESULTS_DIR}"
echo ""
echo "Flamegraphs:"
ls -lh "${RESULTS_DIR}"/flamegraph_*.svg
