#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="$REPO_ROOT/.env"

if [ "$#" -lt 3 ]; then
  echo "=========================================================="
  echo "  GnuCash Web - Admin User Creation"
  echo "=========================================================="
  echo "Usage: $0 <username> <email> <password>"
  echo ""
  echo "Example:"
  echo "  $0 deboshree deboshree@example.com MyPass@123"
  echo "=========================================================="
  exit 1
fi

USERNAME="$1"
EMAIL="$2"
PASSWORD="$3"

if [ -f "$ENV_FILE" ]; then
  ADMIN_KEY=$(grep -E "^ADMIN_API_KEY=" "$ENV_FILE" | cut -d'=' -f2- | tr -d ' "\r\n' || true)
  if [ -z "$ADMIN_KEY" ]; then
    ADMIN_KEY=$(grep -E "^JWT_SECRET=" "$ENV_FILE" | cut -d'=' -f2- | tr -d ' "\r\n' || true)
  fi
else
  echo "❌ Error: .env file not found at $ENV_FILE"
  exit 1
fi

if [ -z "$ADMIN_KEY" ]; then
  echo "❌ Error: ADMIN_API_KEY or JWT_SECRET not found in $ENV_FILE"
  exit 1
fi

echo "Creating user '$USERNAME' ($EMAIL)..."

PAYLOAD=$(jq -n \
  --arg username "$USERNAME" \
  --arg email "$EMAIL" \
  --arg password "$PASSWORD" \
  '{username: $username, email: $email, password: $password}')

RES=$(echo "$PAYLOAD" | curl -s -X POST http://localhost:8081/api/auth/signup \
  -H "Content-Type: application/json" \
  -H "X-Admin-Key: ${ADMIN_KEY}" \
  -d @-)

if echo "$RES" | grep -q '"token"'; then
  echo "✅ User '$USERNAME' successfully created with starter Chart of Accounts!"
  echo "You can now log in at http://localhost:8081 with '$USERNAME'."
else
  ERROR_MSG=$(echo "$RES" | grep -o '"error":"[^"]*"' | cut -d':' -f2- | tr -d '"' || echo "$RES")
  echo "❌ Failed to create user: $ERROR_MSG"
  exit 1
fi
