#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="$REPO_ROOT/.env"

if [ "$#" -lt 1 ]; then
  echo "=========================================================="
  echo "  GnuCash Web - Admin Password Reset"
  echo "=========================================================="
  echo "Usage: $0 <username_or_email> [new_password]"
  echo ""
  echo "Examples:"
  echo "  # 1. Generate random temporary password written to server files:"
  echo "  $0 kishan"
  echo ""
  echo "  # 2. Set specific new password directly:"
  echo "  $0 kishan 'MyNewPass@123'"
  echo "=========================================================="
  exit 1
fi

IDENTIFIER="$1"
CUSTOM_PASSWORD="${2:-}"

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

if [ -n "$CUSTOM_PASSWORD" ]; then
  PAYLOAD="{\"username\":\"$IDENTIFIER\",\"new_password\":\"$CUSTOM_PASSWORD\"}"
else
  PAYLOAD="{\"username\":\"$IDENTIFIER\"}"
fi

echo "Resetting password for '$IDENTIFIER'..."

RES=$(curl -s -X POST http://localhost:8081/api/auth/reset-password \
  -H "Content-Type: application/json" \
  -H "X-Admin-Key: ${JWT_SECRET}" \
  -d "$PAYLOAD")

if echo "$RES" | grep -q '"success":true'; then
  TEMP_PASS=$(echo "$RES" | grep -o '"temp_password":"[^"]*"' | cut -d':' -f2- | tr -d '"' || true)
  IS_TEMP=$(echo "$RES" | grep -o '"is_temporary":true' || true)
  
  echo "=========================================================="
  echo "  ✅ Password Reset Succeeded for '$IDENTIFIER'"
  echo "=========================================================="
  if [ -n "$IS_TEMP" ] && [ -n "$TEMP_PASS" ]; then
    echo "Temporary Password : $TEMP_PASS"
    echo ""
    echo "Next Steps:"
    echo "  1. Log in at http://localhost:8081 with '$IDENTIFIER' and this temporary password."
    echo "  2. Go to User Menu -> Change Password to update to your permanent password."
  else
    echo "Permanent password has been updated as requested."
    echo "You can now log in at http://localhost:8081 with '$IDENTIFIER'."
  fi
  echo "=========================================================="
else
  ERROR_MSG=$(echo "$RES" | grep -o '"error":"[^"]*"' | cut -d':' -f2- | tr -d '"' || echo "$RES")
  echo "❌ Failed to reset password: $ERROR_MSG"
  exit 1
fi
