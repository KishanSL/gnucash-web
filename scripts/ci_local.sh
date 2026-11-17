#!/usr/bin/env bash
# Runs the same checks as .forgejo/workflows/ci.yml on this machine, for use until a Forgejo
# runner is registered (or before pushing):
#
#   backend:  cargo clippy --all-targets -D warnings, cargo test (separate gnucash_test database)
#   frontend: typecheck, ESLint (zero warnings), unit tests, production build
#   e2e:      all Puppeteer suites in the isolated throwaway environment (npm run test:e2e)
#
# Usage:
#   ./scripts/ci_local.sh            # everything
#   ./scripts/ci_local.sh --no-e2e   # skip the Puppeteer suites (about 4 minutes faster)
#
# Needs the local Postgres container (gnucash-postgres) running. Never touches the live database.

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_E2E=true
case "${1:-}" in
    --no-e2e) RUN_E2E=false ;;
    "") ;;
    *) echo "Usage: $0 [--no-e2e]" >&2; exit 2 ;;
esac

if [ -f "$ROOT_DIR/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    . "$ROOT_DIR/.env"
    set +a
fi
# Tests derive gnucash_test from this URL and refuse to use a database not ending in _test
export DATABASE_URL="postgres://${POSTGRES_USER:-gnucash}:${POSTGRES_PASSWORD:-gnucash_secret}@127.0.0.1:${POSTGRES_PORT:-5432}/gnucash_test"

PASSED=()
FAILED=()

step() {
    local name="$1" dir="$2"
    shift 2
    echo
    echo "==> $name"
    if (cd "$ROOT_DIR/$dir" && "$@"); then
        PASSED+=("$name")
    else
        FAILED+=("$name")
    fi
}

step "Backend: clippy"        backend  cargo clippy --all-targets -- -D warnings
step "Backend: tests"         backend  cargo test
step "Frontend: typecheck"    frontend npm run -s typecheck
step "Frontend: ESLint"       frontend npm run -s lint
step "Frontend: unit tests"   frontend npm test
step "Frontend: build"        frontend npm run -s build
if $RUN_E2E; then
    step "E2E: Puppeteer suites" frontend npm run -s test:e2e
fi

echo
echo "==> Local CI: ${#PASSED[@]} passed, ${#FAILED[@]} failed"
for s in "${PASSED[@]}"; do echo "  ✅ $s"; done
for s in "${FAILED[@]}"; do echo "  ❌ $s"; done
[ "${#FAILED[@]}" -eq 0 ]
