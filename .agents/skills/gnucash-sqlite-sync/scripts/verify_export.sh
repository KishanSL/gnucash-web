#!/usr/bin/env bash
set -euo pipefail

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "========================================================"
echo "  GnuCash SQLite Export & Schema Integrity Verification"
echo "========================================================"

# 1. Log in to get auth JWT token
echo "🔑 Logging in with kishan / Kishan@123..."
LOGIN_RESP=$(curl -s -X POST http://localhost:8081/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"kishan","password":"Kishan@123"}')

TOKEN=$(echo "$LOGIN_RESP" | jq -r '.token // empty')

if [ -z "$TOKEN" ]; then
  echo "❌ Login failed! Response: $LOGIN_RESP"
  exit 1
fi

echo "✅ Authenticated successfully."

# 2. Export SQLite database
EXPORT_FILE="$TMP_DIR/test_export.gnucash"
echo "📥 Downloading GnuCash SQLite database from /api/export/gnucash-sqlite..."
curl -s -f -H "Authorization: Bearer $TOKEN" http://localhost:8081/api/export/gnucash-sqlite -o "$EXPORT_FILE"

FILE_SIZE=$(wc -c < "$EXPORT_FILE")
echo "Exported file size: $FILE_SIZE bytes"

if [ "$FILE_SIZE" -lt 1000 ]; then
  echo "❌ Export file is suspiciously small ($FILE_SIZE bytes)!"
  cat "$EXPORT_FILE"
  exit 1
fi

# 3. Check SQLite database integrity & schema via python3 sqlite3
python3 - <<EOF
import sqlite3
import sys

conn = sqlite3.connect("$EXPORT_FILE")
cursor = conn.cursor()

print("🔍 Running PRAGMA integrity_check...")
integrity = cursor.execute("PRAGMA integrity_check;").fetchone()[0]
print("Integrity:", integrity)
if integrity != "ok":
    print("❌ Integrity check failed!", file=sys.stderr)
    sys.exit(1)

print("📋 Verifying GnuCash versions table...")
versions = dict(cursor.execute("SELECT table_name, table_version FROM versions;").fetchall())
for name, ver in sorted(versions.items()):
    print(f" - {name}: {ver}")

gnc_ver = versions.get("Gnucash")
if gnc_ver != 5000017:
    print(f"❌ Expected Gnucash version 5000017 (for 5.17), got {gnc_ver}", file=sys.stderr)
    sys.exit(1)
print(f"✅ Gnucash version verified: {gnc_ver} (GnuCash 5.17+ compatible)")

print("⚖️  Verifying double-entry balance for all transactions...")
cursor.execute("""
    SELECT tx_guid, ROUND(SUM(value_num * 1.0 / value_denom), 4) as balance
    FROM splits
    GROUP BY tx_guid
    HAVING ABS(balance) > 0.0001
""")
imbalances = cursor.fetchall()
if imbalances:
    print(f"❌ Found {len(imbalances)} transactions that do not balance to zero!", file=sys.stderr)
    for tx, bal in imbalances[:5]:
        print(f"  TX {tx}: balance={bal}", file=sys.stderr)
    sys.exit(1)

print("✅ 100% of transactions balance to exactly 0.00!")

accounts_cnt = cursor.execute("SELECT count(*) FROM accounts;").fetchone()[0]
tx_cnt = cursor.execute("SELECT count(*) FROM transactions;").fetchone()[0]
splits_cnt = cursor.execute("SELECT count(*) FROM splits;").fetchone()[0]

print("Summary:")
print(f" - Accounts: {accounts_cnt}")
print(f" - Transactions: {tx_cnt}")
print(f" - Splits: {splits_cnt}")
EOF

echo "========================================================"
echo "  ✅ GnuCash 5.17 SQLite Export Verification Passed!"
echo "========================================================"
