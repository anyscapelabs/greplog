#!/usr/bin/env sh
# Manual dashboard verification — drive traffic at the Go test app so its gin
# middleware emits real spans. The middleware only captures requests that
# actually happen, so run this alongside `go run .` in the go/ dir.
#
# Usage: ./traffic.sh [port]
set -eu

PORT="${1:-8080}"
URL="http://127.0.0.1:${PORT}/orders"
echo "Hitting ${URL} every 0.3s (Ctrl+C to stop)..."
i=0
while true; do
  code=$(curl -s -o /dev/null -w '%{http_code}' "${URL}")
  if [ $((i % 20)) -eq 0 ]; then
    echo "orders -> HTTP ${code}"
  fi
  i=$((i + 1))
  sleep 0.3
done
