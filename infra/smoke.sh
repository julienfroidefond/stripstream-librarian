#!/usr/bin/env bash
set -euo pipefail

BASE_API="${BASE_API:-http://127.0.0.1:7080}"
BASE_INDEXER="${BASE_INDEXER:-http://127.0.0.1:7081}"
BASE_BACKOFFICE="${BASE_BACKOFFICE:-${BASE_ADMIN:-http://127.0.0.1:7082}}"
TOKEN="${API_TOKEN:-stripstream-dev-bootstrap-token}"
# Max seconds to wait for a job to finish
JOB_TIMEOUT="${JOB_TIMEOUT:-120}"

# ─── helpers ────────────────────────────────────────────────────────────────

auth() { curl -fsS -H "Authorization: Bearer $TOKEN" "$@"; }

# Wait for a job (by id) to reach status success or failed.
wait_job() {
  local job_id="$1"
  local label="${2:-job}"
  local waited=0
  while true; do
    local status
    status="$(auth "$BASE_API/index/jobs/$job_id" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))")"
    case "$status" in
      success) echo "[smoke] $label finished: success"; return 0 ;;
      failed)  echo "[smoke] $label finished: FAILED"; return 1 ;;
      cancelled) echo "[smoke] $label finished: cancelled"; return 1 ;;
    esac
    if [ "$waited" -ge "$JOB_TIMEOUT" ]; then
      echo "[smoke] $label timed out after ${JOB_TIMEOUT}s (last status: $status)"; return 1
    fi
    sleep 2; waited=$((waited + 2))
  done
}

# ─── health ──────────────────────────────────────────────────────────────────

echo "[smoke] health checks"
curl -fsS "$BASE_API/health"    >/dev/null
curl -fsS "$BASE_API/ready"     >/dev/null
curl -fsS "$BASE_INDEXER/health" >/dev/null
curl -fsS "$BASE_INDEXER/ready" >/dev/null
curl -fsS "$BASE_BACKOFFICE/health" >/dev/null

# ─── libraries ───────────────────────────────────────────────────────────────

echo "[smoke] list libraries"
auth "$BASE_API/libraries" >/dev/null

# ─── full rebuild (2-phase: discovery + analysis) ────────────────────────────

echo "[smoke] queue full rebuild"
REBUILD_JOB_ID="$(auth -X POST "$BASE_API/index/rebuild" | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")"
echo "[smoke] rebuild job id: $REBUILD_JOB_ID"
wait_job "$REBUILD_JOB_ID" "rebuild"

# ─── verify books have page_count + thumbnail after analysis phase ────────────

echo "[smoke] verify books metadata (page_count + thumbnail)"
BOOKS_JSON="$(auth "$BASE_API/books")"
export BOOKS_JSON
python3 - <<'PY'
import json, os, sys

payload = json.loads(os.environ.get("BOOKS_JSON", "{}"))
items = payload.get("items") or []
if not items:
    print("[smoke] no books found — skipping metadata check")
    sys.exit(0)

missing_page_count  = [b["id"] for b in items if not b.get("page_count")]
missing_thumbnail   = [b["id"] for b in items if not b.get("thumbnail_url")]

if missing_page_count:
    print(f"[smoke] WARN: {len(missing_page_count)} book(s) still missing page_count")
if missing_thumbnail:
    print(f"[smoke] WARN: {len(missing_thumbnail)} book(s) still missing thumbnail")

print(f"[smoke] {len(items)} books, {len(items)-len(missing_page_count)} with page_count, {len(items)-len(missing_thumbnail)} with thumbnail")
PY

# ─── page fetch ──────────────────────────────────────────────────────────────

BOOK_ID="$(python3 - <<'PY'
import json, os
items = json.loads(os.environ.get("BOOKS_JSON", "{}")).get("items") or []
print(items[0]["id"] if items else "")
PY
)"

if [ -n "$BOOK_ID" ]; then
  echo "[smoke] fetch page 1 for book $BOOK_ID"
  auth "$BASE_API/books/$BOOK_ID/pages/1?format=webp&quality=80&width=1080" >/dev/null

  echo "[smoke] fetch thumbnail for book $BOOK_ID"
  auth "$BASE_API/books/$BOOK_ID/thumbnail" >/dev/null
fi

# ─── thumbnail rebuild (handled by indexer, not API) ─────────────────────────

echo "[smoke] thumbnail rebuild job"
THUMB_REBUILD_ID="$(auth -X POST -H "Content-Type: application/json" -d '{}' "$BASE_API/index/thumbnails/rebuild" | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")"
echo "[smoke] thumbnail rebuild job id: $THUMB_REBUILD_ID"
wait_job "$THUMB_REBUILD_ID" "thumbnail_rebuild"

# ─── thumbnail regenerate ────────────────────────────────────────────────────

echo "[smoke] thumbnail regenerate job"
THUMB_REGEN_ID="$(auth -X POST -H "Content-Type: application/json" -d '{}' "$BASE_API/index/thumbnails/regenerate" | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")"
echo "[smoke] thumbnail regenerate job id: $THUMB_REGEN_ID"
wait_job "$THUMB_REGEN_ID" "thumbnail_regenerate"

# ─── route checkup supprimée (doit retourner 404) ────────────────────────────

echo "[smoke] /index/jobs/:id/thumbnails/checkup must be gone (404)"
HTTP_CODE="$(curl -s -o /dev/null -w "%{http_code}" -X POST \
  -H "Authorization: Bearer $TOKEN" \
  "$BASE_API/index/jobs/$REBUILD_JOB_ID/thumbnails/checkup")"
if [ "$HTTP_CODE" = "404" ]; then
  echo "[smoke] checkup route correctly returns 404"
else
  echo "[smoke] FAIL: checkup route returned $HTTP_CODE (expected 404)"
  exit 1
fi

# ─── metrics ─────────────────────────────────────────────────────────────────

echo "[smoke] metrics"
curl -fsS "$BASE_API/metrics" >/dev/null

echo "[smoke] OK"
