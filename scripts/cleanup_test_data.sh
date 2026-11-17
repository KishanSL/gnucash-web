#!/usr/bin/env bash
# One-off cleanup of test data left in the live database by test runs between
# 2026-11-13 and 2026-11-17, before the tests were isolated to `gnucash_test`.
#
# Removes:
#   1. Every user other than `kishan` whose email is @example.com, plus the
#      `tenant_b@localhost` isolation-test user, with all their accounts,
#      transactions and splits.
#   2. Test residue inside `kishan`, created after the initial seed
#      (2026-11-13 14:05 UTC): accounts and transactions whose names come from
#      backend/tests/api_tests.rs and the Puppeteer scripts.
#
# Usage:
#   ./scripts/cleanup_test_data.sh            # dry run: shows what would be deleted, then rolls back
#   ./scripts/cleanup_test_data.sh --apply    # backs up the database, then deletes
#   ./scripts/cleanup_test_data.sh --apply --no-backup
#
# Environment overrides: PG_CONTAINER (default gnucash-postgres),
# BACKUP_DIR (default ~/gnucash-backups).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

if [ -f "$ROOT_DIR/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    . "$ROOT_DIR/.env"
    set +a
fi

PG_CONTAINER="${PG_CONTAINER:-gnucash-postgres}"
PG_USER="${POSTGRES_USER:-gnucash}"
PG_DB="${POSTGRES_DB:-gnucash}"
BACKUP_DIR="${BACKUP_DIR:-$HOME/gnucash-backups}"

APPLY=false
BACKUP=true
for arg in "$@"; do
    case "$arg" in
        --apply) APPLY=true ;;
        --no-backup) BACKUP=false ;;
        *) echo "Usage: $0 [--apply] [--no-backup]" >&2; exit 2 ;;
    esac
done

if [ "$PG_DB" = "gnucash_test" ] || [[ "$PG_DB" == *_test ]]; then
    echo "Refusing to run against a test database ($PG_DB)." >&2
    exit 1
fi

if $APPLY && ! $BACKUP; then
    echo "Skipping backup (--no-backup)."
elif $APPLY; then
    mkdir -p "$BACKUP_DIR"
    chmod 700 "$BACKUP_DIR"
    BACKUP_FILE="$BACKUP_DIR/${PG_DB}_pre_cleanup_$(date +%Y%m%d%H%M%S).sql.gz"
    echo "Backing up $PG_DB to $BACKUP_FILE ..."
    podman exec "$PG_CONTAINER" pg_dump -U "$PG_USER" "$PG_DB" | gzip > "$BACKUP_FILE"
    if [ ! -s "$BACKUP_FILE" ] || ! gzip -t "$BACKUP_FILE"; then
        echo "Backup failed or is empty; aborting without deleting anything." >&2
        exit 1
    fi
    chmod 600 "$BACKUP_FILE"
    echo "Backup written ($(du -h "$BACKUP_FILE" | cut -f1))."
    echo "Restore with: gunzip -c '$BACKUP_FILE' | podman exec -i $PG_CONTAINER psql -U $PG_USER -d <empty database>"
else
    echo "DRY RUN: all deletes run inside a transaction that is rolled back. Pass --apply to delete."
fi
echo

podman exec -i "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -v ON_ERROR_STOP=1 -v apply="$APPLY" -qAt <<'SQL'
BEGIN;

CREATE TEMP TABLE cfg AS
SELECT '2026-11-13 14:05:00+00'::timestamptz AS seed_cutoff;

CREATE TEMP TABLE keep_user AS
SELECT id FROM users WHERE username = 'kishan';

DO $$
BEGIN
    IF (SELECT count(*) FROM keep_user) <> 1 THEN
        RAISE EXCEPTION 'Expected exactly one user named kishan';
    END IF;
END $$;

-- 1. Test users
CREATE TEMP TABLE target_users AS
SELECT id FROM users
WHERE id NOT IN (SELECT id FROM keep_user)
  AND (email LIKE '%@example.com'
       OR (username = 'tenant_b' AND email = 'tenant_b@localhost'));

SELECT 'Not touched (review manually): ' || username || ' <' || email || '>'
FROM users
WHERE id NOT IN (SELECT id FROM keep_user)
  AND id NOT IN (SELECT id FROM target_users);

-- 2. Test residue inside kishan, created after the initial seed
CREATE TEMP TABLE target_kishan_accounts AS
SELECT a.id FROM accounts a
WHERE a.user_id = (SELECT id FROM keep_user)
  AND a.created_at > (SELECT seed_cutoff FROM cfg)
  AND a.name IN (
      'Cycle Account A', 'Cycle Account B', 'Test Parent Acct', 'Test Child Acct',
      'Aditya Birla Mutual Fund',
      'Assets', 'Checking', 'Expenses', 'Food & Groceries', 'HDFC Bank Account', 'Salary'
  );

CREATE TEMP TABLE target_kishan_tx AS
SELECT t.id FROM transactions t
WHERE t.user_id = (SELECT id FROM keep_user)
  AND t.created_at > (SELECT seed_cutoff FROM cfg)
  AND (t.description IN (
          'Test Organic Supermarket Run', 'Multi-split Same Account Tx Updated',
          'Changed Amounts Test', 'Uppercase Y Test', 'Clear Fields Tx Test',
          'Updated Description Typo Fixed', 'Original Reconciled Test Tx',
          'Supermarket Mart', 'May Paycheck')
       OR t.description ~ '^(Vendor|Fund SIP) [0-9]{13}$');

-- Safety checks: abort if anything outside the target set would be affected
DO $$
BEGIN
    -- A transaction touching a target kishan account that is not itself a target
    IF EXISTS (
        SELECT 1 FROM splits s
        WHERE s.account_id IN (SELECT id FROM target_kishan_accounts)
          AND s.transaction_id NOT IN (SELECT id FROM target_kishan_tx)
    ) THEN
        RAISE EXCEPTION 'A non-test transaction uses a target kishan account; review manually';
    END IF;

    -- A child of a target kishan account that is not itself a target (parent delete cascades)
    IF EXISTS (
        SELECT 1 FROM accounts a
        WHERE a.parent_id IN (SELECT id FROM target_kishan_accounts)
          AND a.id NOT IN (SELECT id FROM target_kishan_accounts)
    ) THEN
        RAISE EXCEPTION 'A non-test account sits under a target kishan account; review manually';
    END IF;

    -- Splits crossing between a target user and any other user
    IF EXISTS (
        SELECT 1 FROM splits s
        JOIN transactions t ON t.id = s.transaction_id
        JOIN accounts a ON a.id = s.account_id
        WHERE (t.user_id IN (SELECT id FROM target_users)) <> (a.user_id IN (SELECT id FROM target_users))
    ) THEN
        RAISE EXCEPTION 'Found splits crossing between test users and other users; review manually';
    END IF;
END $$;

SELECT '';
SELECT 'Would delete / deleting:';
SELECT '  test users:                  ' || count(*) FROM target_users;
SELECT '  test-user accounts:          ' || count(*) FROM accounts WHERE user_id IN (SELECT id FROM target_users);
SELECT '  test-user transactions:      ' || count(*) FROM transactions WHERE user_id IN (SELECT id FROM target_users);
SELECT '  kishan test accounts:        ' || count(*) FROM target_kishan_accounts;
SELECT '  kishan test transactions:    ' || count(*) FROM target_kishan_tx;

-- Deletes. Splits go with their transactions (ON DELETE CASCADE);
-- the checks above guarantee no remaining split references a deleted account.
DELETE FROM transactions WHERE id IN (SELECT id FROM target_kishan_tx);
DELETE FROM accounts WHERE id IN (SELECT id FROM target_kishan_accounts);
DELETE FROM transactions WHERE user_id IN (SELECT id FROM target_users);
DELETE FROM accounts WHERE user_id IN (SELECT id FROM target_users);
DELETE FROM users WHERE id IN (SELECT id FROM target_users);

SELECT '';
SELECT 'Remaining after cleanup:';
SELECT '  users:                       ' || count(*) FROM users;
SELECT '  kishan accounts:             ' || count(*) FROM accounts WHERE user_id = (SELECT id FROM keep_user);
SELECT '  kishan transactions:         ' || count(*) FROM transactions WHERE user_id = (SELECT id FROM keep_user);
SELECT '  orphan splits:               ' || count(*) FROM splits s
       WHERE NOT EXISTS (SELECT 1 FROM transactions t WHERE t.id = s.transaction_id);

\if :apply
COMMIT;
\echo
\echo Cleanup committed.
\else
ROLLBACK;
\echo
\echo Dry run complete: rolled back, nothing was deleted.
\endif
SQL
