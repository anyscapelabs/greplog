#!/usr/bin/env bash
set -euo pipefail

RELEASE_DIR="/home/brnx/Desktop/greplog/target/release"
AGENT_BIN="${RELEASE_DIR}/greplog-agent"
BENCH_BIN="${RELEASE_DIR}/bench_throughput"
RESULTS_DIR="/home/brnx/Desktop/greplog/throughput_results"
AGENT_LOG="/tmp/greplog_throughput_agent.log"

mkdir -p "$RESULTS_DIR"

echo "=== Throughput Test with perf profiling (32 producers) ==="
echo ""

producers=32
tag="perf_p32"
workspace="/tmp/greplog_throughput_${tag}"
socket="${workspace}/.greplog/greplog.sock"

# Clean and create workspace
rm -rf "$workspace"
mkdir -p "${workspace}/.greplog"

# Start agent
echo "Starting agent (max_buffer_events=500000)..."
RUST_LOG=info "$AGENT_BIN" \
    --workspace "$workspace" \
    --tcp-port 0 \
    --ui-port 0 \
    --socket-path ".greplog/greplog.sock" \
    --flush-interval-secs 2 \
    --max-buffer-events 500000 \
    > "$AGENT_LOG" 2>&1 &

agent_pid=$!
echo "Agent PID: $agent_pid"

# Wait for UDS socket
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
    exit 1
fi

sleep 2

# Start perf profiling in background
echo "Starting perf record on agent process..."
perf record -F 99 -p "$agent_pid" -g -o "${RESULTS_DIR}/perf_${tag}.data" -- sleep 45 &
perf_pid=$!

sleep 1

echo "Running benchmark: 45 second sustained load test..."
timeout 45 "$BENCH_BIN" \
    --socket "$socket" \
    --status-host "127.0.0.1:0" \
    --start-rate 10000 \
    --rate-step 0 \
    --step-secs 45 \
    --steps 1 \
    --events-per-batch 10 \
    --producers "$producers" \
    --service "bench-${tag}" \
    2>&1 | tee "${RESULTS_DIR}/throughput_${tag}.txt" || true

# Wait for perf to finish
wait $perf_pid 2>/dev/null || true

# Generate perf report
echo ""
echo "Generating perf report..."
perf report -i "${RESULTS_DIR}/perf_${tag}.data" --stdio > "${RESULTS_DIR}/perf_report_${tag}.txt" 2>&1 || true

# Cleanup
kill "$agent_pid" 2>/dev/null || true
wait "$agent_pid" 2>/dev/null || true

echo ""
echo "=== Results ==="
echo "Throughput log: ${RESULTS_DIR}/throughput_${tag}.txt"
echo "Perf data: ${RESULTS_DIR}/perf_${tag}.data"
echo "Perf report: ${RESULTS_DIR}/perf_report_${tag}.txt"
echo ""
echo "Top CPU consumers:"
head -50 "${RESULTS_DIR}/perf_report_${tag}.txt"
