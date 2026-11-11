#!/usr/bin/env bash
set -euo pipefail

# Find repository root
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$REPO_ROOT"

TARGET="${1:-all}"

echo "========================================================"
echo "  GnuCash Web Deployment & Verification (${TARGET})"
echo "========================================================"

if [[ "$TARGET" == "frontend" || "$TARGET" == "all" ]]; then
  echo "📦 [1/4] Building and deploying Frontend..."
  (cd frontend && npm run build)
  podman cp frontend/dist/. gnucash-frontend:/usr/share/nginx/html/
  podman cp frontend/nginx.conf gnucash-frontend:/etc/nginx/conf.d/default.conf
  podman exec gnucash-frontend nginx -t
  podman restart gnucash-frontend
fi

if [[ "$TARGET" == "backend" || "$TARGET" == "all" ]]; then
  echo "🦀 [2/4] Building and deploying Backend..."
  (cd backend && cargo build --release)
  podman cp backend/target/x86_64-unknown-linux-gnu/release/gnucash-backend gnucash-backend:/app/gnucash-backend
  podman restart gnucash-backend
  # Restart frontend so Nginx upstream DNS resolves to updated backend IP
  podman restart gnucash-frontend
fi

echo "🩺 [3/4] Verifying health endpoints..."
sleep 1
BACKEND_HEALTH=$(curl -s http://localhost:8082/api/health || echo "FAILED")
FRONTEND_HEALTH=$(curl -s http://localhost:8081/api/health || echo "FAILED")

echo "Backend Direct (8082):  $BACKEND_HEALTH"
echo "Frontend Proxy (8081):  $FRONTEND_HEALTH"

if [[ "$BACKEND_HEALTH" != *"ok"* || "$FRONTEND_HEALTH" != *"ok"* ]]; then
  echo "❌ Health checks failed!"
  exit 1
fi

echo "🎭 [4/4] Executing Puppeteer Verification Suites..."
(cd frontend && node scripts/test_ui.mjs)
(cd frontend && node scripts/test_register_autocomplete.mjs)
(cd frontend && node scripts/test_reset_password.mjs)

echo "========================================================"
echo "  ✅ All Deployment & Verification Steps Succeeded!"
echo "========================================================"
