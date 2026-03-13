#!/usr/bin/env bash
# perf_pages.sh — Page render performance benchmark
#
# Tests archive reading performance:
#   - Cold render latency per format (CBZ/CBR/PDF, cache-busted widths)
#   - Warm render latency (disk cache hit)
#   - Sequential pages 1-10 (archive open/close overhead)
#   - Concurrent rendering throughput (N simultaneous requests)
#
# Usage:
#   BASE_API=http://localhost:7080 API_TOKEN=my-token bash infra/perf_pages.sh
#
# Optional:
#   BENCH_N    requests per latency measurement (default 10)
#   CONC_N     concurrent requests for throughput test (default 10)
#
# Requires migration 0020 (format column on books table) for per-format tests.

set -euo pipefail

BASE_API="${BASE_API:-http://127.0.0.1:7080}"
TOKEN="${API_TOKEN:-stripstream-dev-bootstrap-token}"
BENCH_N="${BENCH_N:-10}"
CONC_N="${CONC_N:-10}"
export BASE_API TOKEN

BOLD="\033[1m"; RESET="\033[0m"; GREEN="\033[32m"; YELLOW="\033[33m"; CYAN="\033[36m"
header() { echo -e "\n${BOLD}${CYAN}▶ $*${RESET}"; }
ok()     { echo -e "  ${GREEN}✓${RESET} $*"; }
warn()   { echo -e "  ${YELLOW}⚠${RESET} $*"; }
row()    { printf "  %-44s %s\n" "$1" "$2"; }

auth()   { curl -fsS -H "Authorization: Bearer $TOKEN" "$@"; }
now_ms() { python3 -c "import time; print(int(time.time()*1000))"; }

# ─── health ──────────────────────────────────────────────────────────────────

header "Health"
curl -fsS "$BASE_API/health" >/dev/null && ok "API reachable"

BOOKS_JSON="$(auth "$BASE_API/books?limit=100")"
BOOK_COUNT="$(echo "$BOOKS_JSON" | python3 -c "import sys,json; print(json.load(sys.stdin).get('total',0))")"
ok "Books in index: $BOOK_COUNT"

if [ "$BOOK_COUNT" -eq 0 ]; then
  echo "No books found — aborting"; exit 1
fi

# Default benchmark target: first book
FIRST_BOOK_ID="$(echo "$BOOKS_JSON" | python3 -c "
import sys,json; items=json.load(sys.stdin).get('items',[]); print(items[0]['id'] if items else '')
")"
FIRST_BOOK_FORMAT="$(echo "$BOOKS_JSON" | python3 -c "
import sys,json; items=json.load(sys.stdin).get('items',[]); print(items[0].get('format') or '?' if items else '?')
")"
ok "Default target: $FIRST_BOOK_ID  (format: $FIRST_BOOK_FORMAT)"

# One book per format — uses ?format= filter (requires migration 0020)
find_book() {
  local fmt="$1"
  auth "$BASE_API/books?format=$fmt&limit=1" 2>/dev/null \
    | python3 -c "import sys,json; items=json.load(sys.stdin).get('items',[]); print(items[0]['id'] if items else '')" \
    2>/dev/null || echo ""
}
BOOK_CBZ=$(find_book cbz)
BOOK_CBR=$(find_book cbr)
BOOK_PDF=$(find_book pdf)

[ -n "$BOOK_CBZ" ] && ok "CBZ sample: $BOOK_CBZ" || warn "No CBZ (run migration 0020 + rebuild?)"
[ -n "$BOOK_CBR" ] && ok "CBR sample: $BOOK_CBR" || warn "No CBR"
[ -n "$BOOK_PDF" ] && ok "PDF sample: $BOOK_PDF" || warn "No PDF"

# ─── helpers ─────────────────────────────────────────────────────────────────

# Cold render: cycle widths to bypass disk cache
measure_cold() {
  local label="$1" book_id="$2" page="${3:-1}" n="${4:-$BENCH_N}"
  local total=0 i
  for i in $(seq 1 "$n"); do
    local w=$((480 + i))
    local t
    t=$(curl -s -o /dev/null -w '%{time_total}' \
      -H "Authorization: Bearer $TOKEN" \
      "$BASE_API/books/$book_id/pages/$page?format=webp&quality=80&width=$w")
    total=$(python3 -c "print($total + $t)")
  done
  local avg_ms
  avg_ms=$(python3 -c "print(round(($total / $n)*1000, 1))")
  row "$label" "${avg_ms}ms  avg  (cold, n=$n)"
}

# Warm render: prime cache then measure
measure_warm() {
  local label="$1" book_id="$2" n="${3:-$BENCH_N}"
  local url="$BASE_API/books/$book_id/pages/1?format=webp&quality=80&width=600"
  curl -s -o /dev/null -H "Authorization: Bearer $TOKEN" "$url" >/dev/null
  local total=0 i
  for i in $(seq 1 "$n"); do
    local t
    t=$(curl -s -o /dev/null -w '%{time_total}' -H "Authorization: Bearer $TOKEN" "$url")
    total=$(python3 -c "print($total + $t)")
  done
  local avg_ms
  avg_ms=$(python3 -c "print(round(($total / $n)*1000, 1))")
  row "$label" "${avg_ms}ms  avg  (warm/cached, n=$n)"
}

# ─── 1. Cold render by format ────────────────────────────────────────────────

header "1 / Cold Render by Format  (cache-busted, n=$BENCH_N per format)"
if [ -n "$BOOK_CBZ" ]; then
  measure_cold "CBZ  page 1" "$BOOK_CBZ"
else
  warn "skip CBZ"
fi
if [ -n "$BOOK_CBR" ]; then
  measure_cold "CBR  page 1" "$BOOK_CBR"
else
  warn "skip CBR"
fi
if [ -n "$BOOK_PDF" ]; then
  measure_cold "PDF  page 1" "$BOOK_PDF"
else
  warn "skip PDF"
fi

# ─── 2. Warm render ──────────────────────────────────────────────────────────

header "2 / Warm Render  (disk cache, n=$BENCH_N)"
measure_warm "Default book  page 1  ($FIRST_BOOK_FORMAT)" "$FIRST_BOOK_ID"

# ─── 3. Sequential pages ─────────────────────────────────────────────────────

header "3 / Sequential Pages  (pages 1–10, default book, cold widths)"
echo "  book: $FIRST_BOOK_ID  (format: $FIRST_BOOK_FORMAT)"
SEQ_TOTAL=0
for PAGE in $(seq 1 10); do
  T=$(curl -s -o /dev/null -w '%{time_total}' \
    -H "Authorization: Bearer $TOKEN" \
    "$BASE_API/books/$FIRST_BOOK_ID/pages/$PAGE?format=webp&quality=80&width=$((500 + PAGE * 3))")
  MS=$(python3 -c "print(round($T*1000, 1))")
  SEQ_TOTAL=$(python3 -c "print($SEQ_TOTAL + $T)")
  row "  page $PAGE" "${MS}ms"
done
SEQ_AVG=$(python3 -c "print(round($SEQ_TOTAL / 10 * 1000, 1))")
echo "  ──────────────────────────────────────────────────"
row "  avg (10 pages)" "${SEQ_AVG}ms"

# ─── 4. Concurrent throughput ────────────────────────────────────────────────

header "4 / Concurrent Throughput  ($CONC_N simultaneous requests)"
echo "  book: $FIRST_BOOK_ID  (format: $FIRST_BOOK_FORMAT)"
T_START=$(now_ms)
PIDS=()
for i in $(seq 1 "$CONC_N"); do
  curl -s -o /dev/null \
    -H "Authorization: Bearer $TOKEN" \
    "$BASE_API/books/$FIRST_BOOK_ID/pages/$i?format=webp&quality=80&width=$((550 + i * 3))" &
  PIDS+=($!)
done
for PID in "${PIDS[@]}"; do wait "$PID" 2>/dev/null || true; done
T_END=$(now_ms)
CONC_MS=$((T_END - T_START))
CONC_PER=$(python3 -c "print(round($CONC_MS / $CONC_N, 1))")
CONC_TPS=$(python3 -c "print(round($CONC_N / ($CONC_MS / 1000), 1))")

row "  wall time  ($CONC_N pages in parallel)" "${CONC_MS}ms"
row "  avg per page" "${CONC_PER}ms"
row "  throughput" "${CONC_TPS} pages/s"

# ─── 5. Format cross-check ───────────────────────────────────────────────────

if [ -n "$BOOK_CBZ" ] && [ -n "$BOOK_CBR" ] && [ -n "$BOOK_PDF" ]; then
  header "5 / Format Cross-Check  (5 pages each, cold)"
  for PAIR in "CBZ:$BOOK_CBZ" "CBR:$BOOK_CBR" "PDF:$BOOK_PDF"; do
    FMT="${PAIR%%:*}"
    BID="${PAIR##*:}"
    FMT_TOTAL=0
    for PAGE in 1 2 3 4 5; do
      T=$(curl -s -o /dev/null -w '%{time_total}' \
        -H "Authorization: Bearer $TOKEN" \
        "$BASE_API/books/$BID/pages/$PAGE?format=webp&quality=80&width=$((490 + PAGE * 7))")
      FMT_TOTAL=$(python3 -c "print($FMT_TOTAL + $T)")
    done
    AVG=$(python3 -c "print(round($FMT_TOTAL / 5 * 1000, 1))")
    row "$FMT  avg pages 1-5  (cold)" "${AVG}ms"
  done
fi

# ─── done ────────────────────────────────────────────────────────────────────

header "Done"
echo -e "  Run again after the parsers refactoring to compare.\n"
