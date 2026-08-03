#!/usr/bin/env bash
set -euo pipefail

RELEASE_DIR="/home/brnx/Desktop/greplog/target/release"
AGENT_BIN="${RELEASE_DIR}/greplog-agent"
BENCH_BIN="${RELEASE_DIR}/bench_throughput"
RESULTS_DIR="/home/brnx/Desktop/greplog/throughput_results"
AGENT_LOG="/tmp/greplog_throughput_agent.log"

mkdir -p "$RESULTS_DIR"

echo "=== Quick Throughput Test (32 producers, 60 seconds) ==="
echo ""

producers=32
tag="quick_p32"
workspace="/tmp/greplog_throughput_${tag}"
socket="${workspace}/.greplog/greplog.sock"

# Clean and create workspace
rm -rf "$workspace"
mkdir -p "${workspace}/.greplog"

# Start agent with new buffer size
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

# Monitor CPU in background
(
    while kill -0 $agent_pid 2>/dev/null; do
        ps -p $agent_pid -o %cpu,rss,vsz | tail -1
        sleep 5
    done
) > "${RESULTS_DIR}/cpu_${tag}.txt" 2>&1 &
monitor_pid=$!

echo "Running benchmark: start_rate=5000, rate_step=5000, 60 second test..."
"$BENCH_BIN" \
    --socket "$socket" \
    --status-host "127.0.0.1:0" \
    --start-rate 5000 \
    --rate-step 5000 \
    --step-secs 30 \
    --steps 2 \
    --events-per-batch 10 \
    --producers "$producers" \
    --service "bench-${tag}" \
    2>&1 | tee "${RESULTS_DIR}/throughput_${tag}.txt"

# Cleanup
kill $monitor_pid 2>/dev/null || true
kill "$agent_pid" 2>/dev/null || true
wait "$agent_pid" 2>/dev/null || true

echo ""
echo "=== Results ==="
echo "Throughput log: ${RESULTS_DIR}/throughput_${tag}.txt"
echo "CPU usage log: ${RESULTS_DIR}/cpu_${tag}.txt"
echo ""
echo "Final throughput achieved:"
grep -i "accepted\|rate\|events" "${RESULTS_DIR}/throughput_${tag}.txt" | tail -20 || true
