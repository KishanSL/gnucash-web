#!/usr/bin/env bash
# Runs the Puppeteer scripts against an isolated, throwaway environment so that
# no test ever touches the live database:
#
#   1. creates a fresh `gnucash_e2e_test` database
#   2. starts a debug backend on it (127.0.0.1:8092) with a random JWT secret;
#      the backend creates `kishan` / `Kishan@123` and seeds the sample data
#   3. imports a fund account (commodity_fraction = 1000) for test_fund_account_decimals through
#      the app's GnuCash import API (scripts/fixtures/fund_account.gnucash.xml)
#   4. builds the frontend and serves it with `vite preview` (127.0.0.1:8093),
#      proxying /api to the throwaway backend
#   5. runs the scripts, then stops everything and drops the database
#
# Usage:
#   npm run test:e2e                          # all scripts
#   npm run test:e2e -- test_register_enhancements test_ui
#
# Environment overrides: E2E_DB_NAME, E2E_BACKEND_PORT, E2E_FRONTEND_PORT, PG_HOST, PG_CONTAINER.
#
# Database lifecycle: locally the database is created and dropped with `podman exec` in the
# PG_CONTAINER (default gnucash-postgres). In CI, set E2E_DB_PROVISIONED=1 when the Postgres service
# has already created an empty E2E_DB_NAME; it is then neither created nor dropped. No psql client
# is needed in either case.

set -euo pipefail

FRONTEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="$(dirname "$FRONTEND_DIR")"
BACKEND_DIR="$ROOT_DIR/backend"

if [ -f "$ROOT_DIR/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    . "$ROOT_DIR/.env"
    set +a
fi

PG_CONTAINER="${PG_CONTAINER:-gnucash-postgres}"
PG_USER="${POSTGRES_USER:-gnucash}"
PG_PASSWORD="${POSTGRES_PASSWORD:-gnucash_secret}"
PG_PORT="${POSTGRES_PORT:-5432}"
PG_HOST="${PG_HOST:-127.0.0.1}"
E2E_DB_NAME="${E2E_DB_NAME:-gnucash_e2e_test}"
BACKEND_PORT="${E2E_BACKEND_PORT:-8092}"
FRONTEND_PORT="${E2E_FRONTEND_PORT:-8093}"

ALL_TESTS=(
    test_ui
    test_currency_dropdown
    test_tree_collapse
    test_register_autocomplete
    test_autocomplete_autofill
    test_register_enhancements
    test_ui_comprehensive
    test_reports_tab
    test_reconcile_view
    test_fund_account_decimals
    test_export_gnucash
    test_api_tokens
    test_reset_password # last: it resets and restores the password
)
if [ "$#" -gt 0 ]; then
    TESTS=("$@")
else
    TESTS=("${ALL_TESTS[@]}")
fi

for t in "${TESTS[@]}"; do
    [ -f "$FRONTEND_DIR/scripts/$t.mjs" ] || { echo "Unknown test: $t" >&2; exit 2; }
done

if [[ "$E2E_DB_NAME" != *_test ]]; then
    echo "E2E_DB_NAME must end with _test (got '$E2E_DB_NAME')." >&2
    exit 1
fi

for port in "$BACKEND_PORT" "$FRONTEND_PORT"; do
    if curl -s -o /dev/null "http://127.0.0.1:$port"; then
        echo "Port $port is already in use; set E2E_BACKEND_PORT / E2E_FRONTEND_PORT." >&2
        exit 1
    fi
done

if [ "${E2E_DB_PROVISIONED:-0}" = 1 ]; then
    MANAGE_DB=0
elif [ -n "$PG_CONTAINER" ] && command -v podman >/dev/null 2>&1 \
    && podman container exists "$PG_CONTAINER" 2>/dev/null; then
    MANAGE_DB=1
else
    echo "The '$PG_CONTAINER' container is not available. Start it, or set E2E_DB_PROVISIONED=1" >&2
    echo "when $E2E_DB_NAME has already been created (e.g. by a CI Postgres service)." >&2
    exit 1
fi

# Admin SQL (create/drop database) inside the local Postgres container
psql_admin() {
    podman exec -i "$PG_CONTAINER" psql -U "$PG_USER" -d postgres -v ON_ERROR_STOP=1 -qAt -c "$1"
}

WORK_DIR="$(mktemp -d)"
BACKEND_PID=""
PREVIEW_PID=""

cleanup() {
    local status=$?
    [ -n "$PREVIEW_PID" ] && kill -- "-$PREVIEW_PID" 2>/dev/null || true
    [ -n "$BACKEND_PID" ] && kill -- "-$BACKEND_PID" 2>/dev/null || true
    wait 2>/dev/null || true
    if [ "$MANAGE_DB" = 1 ]; then
        psql_admin "DROP DATABASE IF EXISTS \"$E2E_DB_NAME\" WITH (FORCE)" >/dev/null 2>&1 || true
    fi
    rm -rf "$WORK_DIR"
    exit "$status"
}
trap cleanup EXIT INT TERM

show_logs() {
    echo "----- backend log (last 30 lines) -----"
    tail -n 30 "$WORK_DIR/backend.log" 2>/dev/null || true
    echo "----- preview log (last 30 lines) -----"
    tail -n 30 "$WORK_DIR/preview.log" 2>/dev/null || true
}

wait_for() {
    local url="$1" name="$2"
    for _ in $(seq 1 90); do
        if curl -sf -o /dev/null "$url"; then return 0; fi
        sleep 1
    done
    echo "$name did not become ready at $url" >&2
    show_logs
    return 1
}

if [ "$MANAGE_DB" = 1 ]; then
    echo "==> Creating database $E2E_DB_NAME"
    psql_admin "DROP DATABASE IF EXISTS \"$E2E_DB_NAME\" WITH (FORCE)"
    psql_admin "CREATE DATABASE \"$E2E_DB_NAME\""
else
    echo "==> Using pre-provisioned database $E2E_DB_NAME"
fi

echo "==> Building backend (debug)"
(cd "$BACKEND_DIR" && cargo build -q)
BACKEND_BIN="$BACKEND_DIR/target/x86_64-unknown-linux-gnu/debug/gnucash-backend"
[ -x "$BACKEND_BIN" ] || BACKEND_BIN="$BACKEND_DIR/target/debug/gnucash-backend"
[ -x "$BACKEND_BIN" ] || { echo "Backend binary not found after build" >&2; exit 1; }

E2E_JWT_SECRET="$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')"
PG_PASSWORD_ENC="$(python3 -I -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "$PG_PASSWORD")"
mkdir -p "$WORK_DIR/security" "$WORK_DIR/seed"

echo "==> Starting backend on port $BACKEND_PORT"
# Started from WORK_DIR so dotenvy cannot pick up the repository's .env.
(
    cd "$WORK_DIR"
    exec setsid env -u GNUCASH_TEST_MODE \
        DATABASE_URL="postgres://$PG_USER:$PG_PASSWORD_ENC@$PG_HOST:$PG_PORT/$E2E_DB_NAME" \
        BIND_ADDR=127.0.0.1 \
        PORT="$BACKEND_PORT" \
        JWT_SECRET="$E2E_JWT_SECRET" \
        INITIAL_ADMIN_USER=kishan \
        INITIAL_ADMIN_PASSWORD='Kishan@123' \
        ALLOW_SIGNUP=false \
        CORS_ALLOWED_ORIGINS="http://127.0.0.1:$FRONTEND_PORT" \
        GNUCASH_SEED_DIR="$WORK_DIR/seed" \
        SECURITY_DIR="$WORK_DIR/security" \
        RUST_LOG=warn \
        "$BACKEND_BIN"
) >"$WORK_DIR/backend.log" 2>&1 &
BACKEND_PID=$!
wait_for "http://127.0.0.1:$BACKEND_PORT/api/health" "Backend"

echo "==> Importing fund account fixture (commodity_fraction = 1000) via the import API"
API="http://127.0.0.1:$BACKEND_PORT/api"
E2E_TOKEN="$(curl -sf -X POST "$API/auth/login" -H 'Content-Type: application/json' \
    -d '{"username":"kishan","password":"Kishan@123"}' \
    | python3 -I -c 'import json, sys; print(json.load(sys.stdin)["token"])')"
if ! curl -sf -o "$WORK_DIR/import.json" -H "Authorization: Bearer $E2E_TOKEN" \
    -F "file=@$FRONTEND_DIR/scripts/fixtures/fund_account.gnucash.xml" \
    "$API/import/gnucash-sqlite?wipe=false"; then
    echo "Fixture import failed" >&2
    cat "$WORK_DIR/import.json" 2>/dev/null >&2
    show_logs
    exit 1
fi

echo "==> Building frontend"
(cd "$FRONTEND_DIR" && npm run -s build >/dev/null)

echo "==> Starting preview on port $FRONTEND_PORT"
(
    cd "$FRONTEND_DIR"
    exec setsid env API_PROXY_TARGET="http://127.0.0.1:$BACKEND_PORT" \
        npx vite preview --host 127.0.0.1 --port "$FRONTEND_PORT" --strictPort
) >"$WORK_DIR/preview.log" 2>&1 &
PREVIEW_PID=$!
wait_for "http://127.0.0.1:$FRONTEND_PORT/api/health" "Preview"

PASSED=()
FAILED=()
cd "$FRONTEND_DIR"
for t in "${TESTS[@]}"; do
    echo
    echo "==> $t"
    if E2E_BASE_URL="http://127.0.0.1:$FRONTEND_PORT" \
        E2E_ISOLATED=1 \
        GNUCASH_API_URL="http://127.0.0.1:$FRONTEND_PORT" \
        ADMIN_API_KEY="$E2E_JWT_SECRET" \
        node "scripts/$t.mjs"; then
        PASSED+=("$t")
    else
        FAILED+=("$t")
    fi
done

echo
echo "==> Results: ${#PASSED[@]} passed, ${#FAILED[@]} failed"
for t in "${PASSED[@]}"; do echo "  ✅ $t"; done
for t in "${FAILED[@]}"; do echo "  ❌ $t"; done

if [ "${#FAILED[@]}" -gt 0 ]; then
    show_logs
    exit 1
fi
