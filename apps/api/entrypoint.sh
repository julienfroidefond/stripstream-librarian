#!/bin/sh
set -e

# psql requires "postgresql://" but Rust/sqlx accepts both "postgres://" and "postgresql://"
PSQL_URL=$(echo "$DATABASE_URL" | sed 's|^postgres://|postgresql://|')

# Check 1: does the old schema exist (index_jobs table)?
HAS_OLD_TABLES=$(psql "$PSQL_URL" -tAc \
    "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name='index_jobs')::text" \
    2>/dev/null || echo "false")

# Check 2: is sqlx tracking present and non-empty?
HAS_SQLX_TABLE=$(psql "$PSQL_URL" -tAc \
    "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name='_sqlx_migrations')::text" \
    2>/dev/null || echo "false")

if [ "$HAS_SQLX_TABLE" = "true" ]; then
    HAS_SQLX_ROWS=$(psql "$PSQL_URL" -tAc \
        "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations LIMIT 1)::text" \
        2>/dev/null || echo "false")
else
    HAS_SQLX_ROWS="false"
fi

echo "==> Migration check: old_tables=$HAS_OLD_TABLES sqlx_table=$HAS_SQLX_TABLE sqlx_rows=$HAS_SQLX_ROWS"

if [ "$HAS_OLD_TABLES" = "true" ] && [ "$HAS_SQLX_ROWS" = "false" ]; then
    echo "==> Upgrade from pre-sqlx migration system detected: creating baseline..."

    psql "$PSQL_URL" -c "
        CREATE TABLE IF NOT EXISTS _sqlx_migrations (
            version          BIGINT      PRIMARY KEY,
            description      TEXT        NOT NULL,
            installed_on     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            success          BOOLEAN     NOT NULL,
            checksum         BYTEA       NOT NULL,
            execution_time   BIGINT      NOT NULL
        )
    "

    for f in /app/migrations/*.sql; do
        filename=$(basename "$f")
        # Strip leading zeros to get the integer version (e.g. "0005" -> "5")
        version=$(echo "$filename" | sed 's/^0*//' | cut -d'_' -f1)
        description=$(echo "$filename" | sed 's/^[0-9]*_//' | sed 's/\.sql$//')
        checksum=$(sha384sum "$f" | awk '{print $1}')

        psql "$PSQL_URL" -c "
            INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time)
            VALUES ($version, '$description', NOW(), TRUE, decode('$checksum', 'hex'), 0)
            ON CONFLICT (version) DO NOTHING
        "
        echo "    baselined: $filename"
    done

    echo "==> Baseline complete."
fi

echo "==> Running migrations..."
sqlx migrate run --source /app/migrations

echo "==> Starting API..."
exec /usr/local/bin/api
