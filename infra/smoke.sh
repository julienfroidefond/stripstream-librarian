#!/usr/bin/env bash
set -euo pipefail

BASE_API="${BASE_API:-http://127.0.0.1:7080}"
BASE_INDEXER="${BASE_INDEXER:-http://127.0.0.1:7081}"
BASE_BACKOFFICE="${BASE_BACKOFFICE:-${BASE_ADMIN:-http://127.0.0.1:7082}}"
TOKEN="${API_TOKEN:-stripstream-dev-bootstrap-token}"

echo "[smoke] health checks"
curl -fsS "$BASE_API/health" >/dev/null
curl -fsS "$BASE_API/ready" >/dev/null
curl -fsS "$BASE_INDEXER/health" >/dev/null
curl -fsS "$BASE_INDEXER/ready" >/dev/null
curl -fsS "$BASE_BACKOFFICE/health" >/dev/null

echo "[smoke] list libraries"
curl -fsS -H "Authorization: Bearer $TOKEN" "$BASE_API/libraries" >/dev/null

echo "[smoke] queue rebuild"
curl -fsS -X POST -H "Authorization: Bearer $TOKEN" "$BASE_API/index/rebuild" >/dev/null
sleep 2

echo "[smoke] list books and optional page fetch"
BOOKS_JSON="$(curl -fsS -H "Authorization: Bearer $TOKEN" "$BASE_API/books")"
BOOK_ID="$(BOOKS_JSON="$BOOKS_JSON" python3 - <<'PY'
import json
import os

payload = json.loads(os.environ.get("BOOKS_JSON", "{}"))
items = payload.get("items") or []
print(items[0]["id"] if items else "")
PY
)"

if [ -n "$BOOK_ID" ]; then
  curl -fsS -H "Authorization: Bearer $TOKEN" "$BASE_API/books/$BOOK_ID/pages/1?format=webp&quality=80&width=1080" >/dev/null
fi

echo "[smoke] metrics"
curl -fsS "$BASE_API/metrics" >/dev/null

echo "[smoke] OK"
