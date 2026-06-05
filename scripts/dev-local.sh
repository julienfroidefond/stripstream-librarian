#!/bin/bash
set -e

# ─── Config ─────────────────────────────────────────────────────────────────
DB_URL="postgres://stripstream:stripstream@localhost:6432/stripstream"
ROOT=$(cd "$(dirname "$0")/.." && pwd)
export PATH="$HOME/.cargo/bin:$PATH"

export DATABASE_URL="$DB_URL"
export API_BASE_URL="http://localhost:7080"

# Override les chemins conteneur → local si définis dans .env
if [ -f "$ROOT/.env" ]; then
  set -a; source "$ROOT/.env"; set +a
fi
export DATABASE_URL="$DB_URL"     # priorité sur le .env (postgres → localhost)
export API_BASE_URL="http://localhost:7080"

# ─── Postgres ────────────────────────────────────────────────────────────────
echo "==> Starting postgres..."
docker compose -f "$ROOT/docker-compose.yml" up -d postgres

echo -n "    Waiting for postgres"
until docker compose -f "$ROOT/docker-compose.yml" exec -T postgres \
  pg_isready -U stripstream -d stripstream &>/dev/null; do
  echo -n "."; sleep 1
done
echo " ready"

# ─── Migrations ──────────────────────────────────────────────────────────────
echo "==> Running migrations..."
sqlx migrate run --source "$ROOT/infra/migrations"

# ─── Compilation ─────────────────────────────────────────────────────────────
echo "==> Building (debug)..."
cargo build -p api -p indexer 2>&1

# ─── Lancement ───────────────────────────────────────────────────────────────
echo "==> Starting API  → http://localhost:7080"
cargo run -p api &
API_PID=$!

echo "==> Starting Indexer → http://localhost:7081"
cargo run -p indexer &
INDEXER_PID=$!

cleanup() {
  echo ""
  echo "==> Stopping services..."
  kill "$API_PID" "$INDEXER_PID" 2>/dev/null || true
  wait "$API_PID" "$INDEXER_PID" 2>/dev/null || true
  echo "    Done. Postgres still running (docker compose stop postgres to kill it)."
}
trap cleanup INT TERM

echo ""
echo "Services running. Ctrl+C to stop."
wait "$API_PID" "$INDEXER_PID"
