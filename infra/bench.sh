#!/usr/bin/env bash
set -euo pipefail

BASE_API="${BASE_API:-http://127.0.0.1:7080}"
TOKEN="${API_TOKEN:-stripstream-dev-bootstrap-token}"

measure() {
  local name="$1"
  local url="$2"
  local total=0
  local n=15
  for _ in $(seq 1 "$n"); do
    local t
    t=$(curl -s -o /dev/null -w '%{time_total}' -H "Authorization: Bearer $TOKEN" "$url")
    total=$(python3 - "$total" "$t" <<'PY'
import sys
print(float(sys.argv[1]) + float(sys.argv[2]))
PY
)
  done
  local avg
  avg=$(python3 - "$total" "$n" <<'PY'
import sys
print(round((float(sys.argv[1]) / int(sys.argv[2]))*1000, 2))
PY
)
  echo "$name avg_ms=$avg"
}

measure "books" "$BASE_API/books"
measure "search" "$BASE_API/search?q=sample"

echo "bench done"
