#!/usr/bin/env bash
# perf.sh — Performance benchmarks for Stripstream Librarian
#
# Measures:
#   - Indexer: full rebuild phase durations (discovery / extracting_pages / generating_thumbnails)
#   - Indexer: incremental rebuild speed (should skip unchanged dirs via mtime cache)
#   - Indexer: thumbnail rebuild (generate missing) and regenerate (force all)
#   - API:     page render latency (cold + warm/cached), thumbnail fetch, books list, search
#
# Usage:
#   BASE_API=http://localhost:7080 API_TOKEN=my-token bash infra/perf.sh
#
# Optional env:
#   JOB_TIMEOUT   seconds to wait for a job to complete (default 600)
#   BENCH_N       number of API requests per endpoint for latency measurement (default 10)
#   LIBRARY_ID    restrict rebuild jobs to a specific library UUID

set -euo pipefail

BASE_API="${BASE_API:-http://127.0.0.1:7080}"
TOKEN="${API_TOKEN:-stripstream-dev-bootstrap-token}"
JOB_TIMEOUT="${JOB_TIMEOUT:-600}"
BENCH_N="${BENCH_N:-10}"
LIBRARY_ID="${LIBRARY_ID:-}"
export BASE_API TOKEN

# ─── colours ────────────────────────────────────────────────────────────────

BOLD="\033[1m"; RESET="\033[0m"; GREEN="\033[32m"; YELLOW="\033[33m"; CYAN="\033[36m"; RED="\033[31m"
header()  { echo -e "\n${BOLD}${CYAN}▶ $*${RESET}"; }
ok()      { echo -e "  ${GREEN}✓${RESET} $*"; }
warn()    { echo -e "  ${YELLOW}⚠${RESET} $*"; }
fail()    { echo -e "  ${RED}✗${RESET} $*"; }
row()     { printf "  %-40s %s\n" "$1" "$2"; }

# ─── helpers ────────────────────────────────────────────────────────────────

auth() { curl -fsS -H "Authorization: Bearer $TOKEN" "$@"; }

# Wait for job to finish; print a dot every 2s.
wait_job() {
  local job_id="$1" label="${2:-job}" waited=0 status
  printf "  waiting for %s ." "$label"
  while true; do
    status="$(auth "$BASE_API/index/jobs/$job_id" \
      | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))")"
    case "$status" in
      success)   echo " done"; return 0 ;;
      failed)    echo " FAILED"; fail "$label failed"; return 1 ;;
      cancelled) echo " cancelled"; fail "$label was cancelled"; return 1 ;;
    esac
    if [ "$waited" -ge "$JOB_TIMEOUT" ]; then
      echo " timeout"; fail "$label timed out after ${JOB_TIMEOUT}s (last: $status)"; return 1
    fi
    printf "."; sleep 2; waited=$((waited + 2))
  done
}

# Fetch /index/jobs/:id/details and pretty-print phase durations + throughput.
report_job() {
  local job_id="$1" label="$2"
  local details
  details="$(auth "$BASE_API/index/jobs/$job_id")"
  export PERF_DETAILS="$details" PERF_LABEL="$label"
  python3 - <<'PY'
import json, os
from datetime import datetime, timezone

def parse(s):
    if not s: return None
    # Handle both with and without microseconds
    for fmt in ("%Y-%m-%dT%H:%M:%S.%fZ", "%Y-%m-%dT%H:%M:%SZ",
                "%Y-%m-%dT%H:%M:%S.%f+00:00", "%Y-%m-%dT%H:%M:%S+00:00"):
        try: return datetime.strptime(s, fmt).replace(tzinfo=timezone.utc)
        except ValueError: pass
    return None

d = json.loads(os.environ["PERF_DETAILS"])
label = os.environ["PERF_LABEL"]

started   = parse(d.get("started_at"))
phase2    = parse(d.get("phase2_started_at"))
thumbs    = parse(d.get("generating_thumbnails_started_at"))
finished  = parse(d.get("finished_at"))
stats     = d.get("stats_json") or {}
total_files = d.get("total_files") or 0

def secs(a, b):
    if a and b: return (b - a).total_seconds()
    return None

def fmt(s):
    if s is None: return "n/a"
    if s < 1: return f"{s*1000:.0f}ms"
    return f"{s:.1f}s"

def tps(n, s):
    if n and s and s > 0: return f"{n/s:.1f}/s"
    return "n/a"

t_total    = secs(started, finished)
t_discover = secs(started, phase2)
t_extract  = secs(phase2, thumbs)
t_thumbs   = secs(thumbs, finished)
indexed    = stats.get("indexed_files", 0)

print(f"  {'Total':38s} {fmt(t_total)}")
if t_discover is not None:
    print(f"  {'  Phase 1 – discovery':38s} {fmt(t_discover)}  ({tps(indexed, t_discover)} books indexed)")
if t_extract is not None:
    print(f"  {'  Phase 2A – extracting_pages':38s} {fmt(t_extract)}  ({tps(total_files, t_extract)} books/s)")
if t_thumbs is not None:
    print(f"  {'  Phase 2B – generating_thumbnails':38s} {fmt(t_thumbs)}  ({tps(total_files, t_thumbs)} thumbs/s)")
print(f"  {'  Files indexed':38s} {indexed} / {total_files}")
if stats.get("errors"):
    print(f"  {'  Errors':38s} {stats['errors']}")
PY
}

# Measure avg latency of a GET endpoint over N requests.
measure_latency() {
  local label="$1" url="$2" n="${3:-$BENCH_N}"
  local total=0 i
  for i in $(seq 1 "$n"); do
    local t
    t=$(curl -s -o /dev/null -w '%{time_total}' -H "Authorization: Bearer $TOKEN" "$url")
    total=$(python3 -c "print($total + $t)")
  done
  local avg_ms
  avg_ms=$(python3 -c "print(round(($total / $n)*1000, 1))")
  row "$label" "${avg_ms}ms  (n=$n)"
}

# Build optional library_id JSON fragment
lib_json() {
  if [ -n "$LIBRARY_ID" ]; then echo "\"library_id\":\"$LIBRARY_ID\","; else echo ""; fi
}

enqueue_rebuild() {
  local full="${1:-false}"
  auth -X POST -H "Content-Type: application/json" \
    -d "{$(lib_json)\"full\":$full}" \
    "$BASE_API/index/rebuild" \
    | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])"
}

enqueue_thumb_rebuild() {
  auth -X POST -H "Content-Type: application/json" \
    -d "{$(lib_json | sed 's/,$//')}" \
    "$BASE_API/index/thumbnails/rebuild" \
    | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])"
}

enqueue_thumb_regen() {
  auth -X POST -H "Content-Type: application/json" \
    -d "{$(lib_json | sed 's/,$//')}" \
    "$BASE_API/index/thumbnails/regenerate" \
    | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])"
}

# ─── health check ────────────────────────────────────────────────────────────

header "Health"
curl -fsS "$BASE_API/health" >/dev/null && ok "API healthy"

BOOKS_JSON="$(auth "$BASE_API/books")"
BOOK_COUNT="$(echo "$BOOKS_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('total',0))")"
FIRST_BOOK_ID="$(echo "$BOOKS_JSON" | python3 -c "import sys,json; items=json.load(sys.stdin).get('items',[]); print(items[0]['id'] if items else '')")"
ok "Books in index: $BOOK_COUNT"
if [ -n "$LIBRARY_ID" ]; then ok "Scoped to library: $LIBRARY_ID"; fi

# ─── 1. full rebuild ─────────────────────────────────────────────────────────

header "1 / Full Rebuild"
JOB_FULL="$(enqueue_rebuild true)"
ok "job $JOB_FULL"
wait_job "$JOB_FULL" "full rebuild"
report_job "$JOB_FULL" "full rebuild"

# ─── 2. incremental rebuild (dirs unchanged → mtime skip) ───────────────────

header "2 / Incremental Rebuild  (should be fast — mtime cache)"
JOB_INCR="$(enqueue_rebuild false)"
ok "job $JOB_INCR"
wait_job "$JOB_INCR" "incremental rebuild"
report_job "$JOB_INCR" "incremental rebuild"

python3 - <<'PY'
import json, os
from datetime import datetime, timezone

def parse(s):
    if not s: return None
    for fmt in ("%Y-%m-%dT%H:%M:%S.%fZ", "%Y-%m-%dT%H:%M:%SZ",
                "%Y-%m-%dT%H:%M:%S.%f+00:00", "%Y-%m-%dT%H:%M:%S+00:00"):
        try: return datetime.strptime(s, fmt).replace(tzinfo=timezone.utc)
        except ValueError: pass
    return None

full_id = os.environ.get("PERF_FULL_ID", "")
incr_id = os.environ.get("PERF_INCR_ID", "")
if not full_id or not incr_id:
    exit(0)
PY
# Speedup ratio via env export
export PERF_FULL_ID="$JOB_FULL" PERF_INCR_ID="$JOB_INCR"
python3 - <<'PY'
import json, os, subprocess
from datetime import datetime, timezone

def parse(s):
    if not s: return None
    for fmt in ("%Y-%m-%dT%H:%M:%S.%fZ", "%Y-%m-%dT%H:%M:%SZ",
                "%Y-%m-%dT%H:%M:%S.%f+00:00", "%Y-%m-%dT%H:%M:%S+00:00"):
        try: return datetime.strptime(s, fmt).replace(tzinfo=timezone.utc)
        except ValueError: pass
    return None

base  = os.environ.get("BASE_API", "http://127.0.0.1:7080")
token = os.environ.get("TOKEN", "")

import urllib.request

def fetch(url):
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {token}"})
    with urllib.request.urlopen(req) as r:
        return json.loads(r.read())

def duration(job_id):
    d = fetch(f"{base}/index/jobs/{job_id}")
    s = parse(d.get("started_at"))
    f = parse(d.get("finished_at"))
    if s and f: return (f - s).total_seconds()
    return None

t_full = duration(os.environ["PERF_FULL_ID"])
t_incr = duration(os.environ["PERF_INCR_ID"])

if t_full and t_incr:
    ratio = t_full / t_incr if t_incr > 0 else 0
    print(f"  {'Speedup (full vs incremental)':38s} {ratio:.1f}x  ({t_full:.1f}s → {t_incr:.1f}s)")
PY

# ─── 3. thumbnail rebuild (generate missing) ─────────────────────────────────

header "3 / Thumbnail Rebuild  (generate missing only)"
JOB_TREB="$(enqueue_thumb_rebuild)"
ok "job $JOB_TREB"
wait_job "$JOB_TREB" "thumbnail rebuild"
report_job "$JOB_TREB" "thumbnail rebuild"

# ─── 4. thumbnail regenerate (force all) ─────────────────────────────────────

header "4 / Thumbnail Regenerate  (force all)"
JOB_TREG="$(enqueue_thumb_regen)"
ok "job $JOB_TREG"
wait_job "$JOB_TREG" "thumbnail regenerate"
report_job "$JOB_TREG" "thumbnail regenerate"

# ─── 5. API latency ──────────────────────────────────────────────────────────

header "5 / API Latency  (n=$BENCH_N requests each)"

measure_latency "books list" "$BASE_API/books"
measure_latency "search (query)" "$BASE_API/search?q=marvel"

if [ -n "$FIRST_BOOK_ID" ]; then
  # Cold page render: clear cache between runs by using different params
  measure_latency "page render  (width=1080, webp)" \
    "$BASE_API/books/$FIRST_BOOK_ID/pages/1?format=webp&quality=80&width=1080"

  # Warm render: same URL repeated → should hit LRU cache
  measure_latency "page render  (warm/cached)" \
    "$BASE_API/books/$FIRST_BOOK_ID/pages/1?format=webp&quality=80&width=1080"

  measure_latency "thumbnail fetch" \
    "$BASE_API/books/$FIRST_BOOK_ID/thumbnail"
else
  warn "No books found — skipping page/thumbnail latency tests"
fi

# ─── 6. Page render deep-dive ────────────────────────────────────────────────
#
#  Tests what the refactoring touches: archive reading for each format.
#  Uses width-cycling to bypass disk cache and measure real decode cost.
#  Tests: per-format cold render, sequential pages, concurrent throughput.

header "6 / Page Render Deep-Dive"

if [ -z "$FIRST_BOOK_ID" ]; then
  warn "No books found — skipping deep-dive"
else

  # Resolve one book per format (API may not support ?format= filter; graceful fallback)
  resolve_book_by_format() {
    local fmt="$1"
    local id
    id=$(auth "$BASE_API/books?format=$fmt&limit=1" 2>/dev/null \
      | python3 -c "import sys,json; items=json.load(sys.stdin).get('items',[]); print(items[0]['id'] if items else '')" 2>/dev/null || echo "")
    echo "$id"
  }
  BOOK_CBZ=$(resolve_book_by_format cbz)
  BOOK_CBR=$(resolve_book_by_format cbr)
  BOOK_PDF=$(resolve_book_by_format pdf)

  # Cold render: cycle widths (480..487) across N requests so each misses disk cache
  measure_latency_cold() {
    local label="$1" book_id="$2" n="${3:-$BENCH_N}"
    local total=0 i
    for i in $(seq 1 "$n"); do
      local w=$((480 + i))    # unique width → unique cache key
      local t
      t=$(curl -s -o /dev/null -w '%{time_total}' \
        -H "Authorization: Bearer $TOKEN" \
        "$BASE_API/books/$book_id/pages/1?format=webp&quality=80&width=$w")
      total=$(python3 -c "print($total + $t)")
    done
    local avg_ms
    avg_ms=$(python3 -c "print(round(($total / $n)*1000, 1))")
    row "$label" "${avg_ms}ms  (cold, n=$n)"
  }

  echo ""
  echo "  Cold render latency by format  (cache-busted widths):"
  [ -n "$BOOK_CBZ" ] && measure_latency_cold "CBZ page 1  (cold)" "$BOOK_CBZ" \
    || warn "No CBZ book found"
  [ -n "$BOOK_CBR" ] && measure_latency_cold "CBR page 1  (cold)" "$BOOK_CBR" \
    || warn "No CBR book found"
  [ -n "$BOOK_PDF" ] && measure_latency_cold "PDF page 1  (cold)" "$BOOK_PDF" \
    || warn "No PDF book found"

  # Warm render: same URL repeated → LRU / disk cache
  echo ""
  echo "  Warm render (disk cache, same URL):"
  # One cold request first, then N warm
  curl -s -o /dev/null -H "Authorization: Bearer $TOKEN" \
    "$BASE_API/books/$FIRST_BOOK_ID/pages/1?format=webp&quality=80&width=600" >/dev/null
  measure_latency "page render (warm/disk-cached)" \
    "$BASE_API/books/$FIRST_BOOK_ID/pages/1?format=webp&quality=80&width=600"

  # Sequential pages: measures archive open+close overhead across consecutive pages
  echo ""
  echo "  Sequential pages (pages 1–10, same book, cold widths):"
  SEQ_TOTAL=0
  for PAGE in $(seq 1 10); do
    local_t=$(curl -s -o /dev/null -w '%{time_total}' \
      -H "Authorization: Bearer $TOKEN" \
      "$BASE_API/books/$FIRST_BOOK_ID/pages/$PAGE?format=webp&quality=80&width=$((500 + PAGE))")
    local_ms=$(python3 -c "print(round($local_t*1000, 1))")
    SEQ_TOTAL=$(python3 -c "print($SEQ_TOTAL + $local_t)")
    row "  page $PAGE" "${local_ms}ms"
  done
  SEQ_AVG=$(python3 -c "print(round($SEQ_TOTAL / 10 * 1000, 1))")
  row "  avg (10 pages)" "${SEQ_AVG}ms"

  # Concurrent throughput: N requests in parallel → measures semaphore + CPU saturation
  CONC_N="${CONC_N:-10}"
  echo ""
  echo "  Concurrent rendering ($CONC_N simultaneous requests, cold widths):"
  CONC_START=$(date +%s%3N)
  PIDS=()
  for i in $(seq 1 "$CONC_N"); do
    curl -s -o /dev/null \
      -H "Authorization: Bearer $TOKEN" \
      "$BASE_API/books/$FIRST_BOOK_ID/pages/$i?format=webp&quality=80&width=$((550 + i))" &
    PIDS+=($!)
  done
  for PID in "${PIDS[@]}"; do wait "$PID" 2>/dev/null || true; done
  CONC_END=$(date +%s%3N)
  CONC_MS=$((CONC_END - CONC_START))
  CONC_PER=$(python3 -c "print(round($CONC_MS / $CONC_N, 1))")
  row "  wall time (${CONC_N} pages in parallel)" "${CONC_MS}ms  (~${CONC_PER}ms/page)"

fi

# ─── summary ─────────────────────────────────────────────────────────────────

header "Summary"
ok "Full rebuild job:          $JOB_FULL"
ok "Incremental rebuild job:   $JOB_INCR"
ok "Thumbnail rebuild job:     $JOB_TREB"
ok "Thumbnail regenerate job:  $JOB_TREG"
echo -e "\n${BOLD}perf done${RESET}"
