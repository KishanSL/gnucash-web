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
  JWT_SECRET=$(grep -E "^JWT_SECRET=" "$ENV_FILE" | cut -d'=' -f2- | tr -d ' "\r\n')
else
  echo "❌ Error: .env file not found at $ENV_FILE"
  exit 1
fi

if [ -z "$JWT_SECRET" ]; then
  echo "❌ Error: JWT_SECRET not found in $ENV_FILE"
  exit 1
fi

echo "Creating user '$USERNAME' ($EMAIL)..."

RES=$(curl -s -X POST http://localhost:8081/api/auth/signup \
  -H "Content-Type: application/json" \
  -H "X-Admin-Key: ${JWT_SECRET}" \
  -d "{\"username\":\"$USERNAME\",\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}")

if echo "$RES" | grep -q '"token"'; then
  echo "✅ User '$USERNAME' successfully created with starter Chart of Accounts!"
  echo "You can now log in at http://localhost:8081 with '$USERNAME'."
else
  ERROR_MSG=$(echo "$RES" | grep -o '"error":"[^"]*"' | cut -d':' -f2- | tr -d '"' || echo "$RES")
  echo "❌ Failed to create user: $ERROR_MSG"
  exit 1
fi
