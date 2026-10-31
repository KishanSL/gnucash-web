# GnuCash Web — Agent Context & Rules

This file provides context, architectural constraints, and mandatory verification protocols for agents working in this repository.

---

## 1. Project Overview & Architecture

**GnuCash Web** is a multi-user, web-based double-entry accounting engine providing desktop-class ledger integrity in a modern browser interface.

- **Frontend**: React 18, Vite, TypeScript, Tailwind CSS (Nginx container on port `8081`)
- **Backend**: Rust, Axum, SQLx, PostgreSQL (container on port `8082`)
- **Database**: PostgreSQL 16 (container on port `5432`)
- **Container Environment**: Podman (aliased to `docker` and `docker compose` via `podman-compose`)

---

## 2. Mandatory UI Verification Protocol (Puppeteer)

> [!IMPORTANT]
> **Always test UI changes with Puppeteer before completing a task.**
> Multi-modal AI models can and should capture and inspect screenshots to prevent UI regressions.

### How to Run Puppeteer Tests
A Puppeteer testing suite is configured in `frontend/scripts/test_ui.mjs` (and available via the Puppeteer MCP server).

- **Default Test Credentials**: `kishan` / `Kishan@123`
- **Frontend URL**: `http://localhost:8081`
- **Run the test script**:
  ```bash
  cd frontend && node scripts/test_ui.mjs
  ```
- **Screenshots Directory**: `frontend/screenshots/`

### Key Viewport Requirements
- **Desktop (1440×900 or 1920×1080)**:
  - Auth page must use a spacious two-column layout (branding & 2×2 feature showcase on the left, login/signup card on the right).
  - Never allow desktop auth pages to collapse into a narrow, mobile-sized single card.
- **Mobile (375×667)**:
  - Must gracefully collapse to a clean, focused, touch-friendly single card.

---

## 3. UI Structure & Navigation Guidelines

- **Main Navigation Bar**:
  - Keep the desktop header clean. It contains only:
    - **New Account** button
    - **New Entry** (transaction) button
    - **User Account Menu Dropdown**
- **User Account Menu Dropdown**:
  - All account, database, credential, and display preferences live inside the user dropdown:
    1. User Profile info (`username`, `email`)
    2. **Currency Preference** (relocated from header into dropdown for cleaner aesthetics)
    3. **Import .gnucash** (opens GnuCash SQLite / XML import modal — *never place on main navbar*)
    4. **Export .gnucash** (downloads database as standard SQLite file formatted `username_yyyyMMddhhmmss.gnucash`)
    5. **Seed Sample Data** (seeds chart of accounts & realistic sample transactions)
    6. **Change Password** (opens credentials modal)
    7. **Reset Entire Database** (warning amber button with `RotateCcw` icon)
    8. **Log Out**
- **Mobile Slide-down Drawer**:
  - Must mirror all actions and preferences available in the desktop user dropdown.

---

## 4. Database & Accounting Rules

1. **Strict Double-Entry Enforcement**:
   - Every transaction must balance: $\sum \text{Debits} + \sum \text{Credits} = 0$.
   - At least 2 splits per transaction.
   - Placeholder accounts (`placeholder = true`) cannot hold direct splits.
2. **Multi-Tenant Isolation**:
   - Every query to `accounts`, `transactions`, and `splits` must strictly filter by `user_id = auth.id`.
3. **Reset Entire Database (`POST /api/reset-data`)**:
   - Permanently deletes **ALL** transactions and **ALL** accounts for the authenticated user.
   - Leaves the database completely empty ($0$ accounts, $0$ transactions) so the user can import a file or create accounts from scratch.
4. **Seed Database (`POST /api/seed`)**:
   - Cleans existing user data and provisions the full 37-account standard Chart of Accounts with realistic sample journal transactions.

---

## 5. Build & Deployment Workflows

Because services run in Podman containers, rebuild and redeploy after code changes:

### Frontend Updates
```bash
cd frontend && npm run build
podman cp dist/. gnucash-frontend:/usr/share/nginx/html/
podman build -t gnucash-frontend:latest ./frontend
```

### Backend Updates
```bash
cd backend && cargo build --release
podman cp ./target/x86_64-unknown-linux-gnu/release/gnucash-backend gnucash-backend:/app/gnucash-backend
podman restart gnucash-backend
# Restart frontend so Nginx upstream DNS resolves to the refreshed backend container IP
podman restart gnucash-frontend
```

### Health Check Verification
```bash
curl -s http://localhost:8082/api/health
curl -s http://localhost:8081/api/health
```

---

## 6. Key API Endpoints Reference

| Endpoint | Method | Description |
| :--- | :--- | :--- |
| `/api/health` | `GET` | Health check (`service`, `status`, `version`) |
| `/api/auth/login` | `POST` | User login (returns JWT token and user info) |
| `/api/auth/signup` | `POST` | User registration |
| `/api/auth/reset-password` | `POST` | Admin/recovery password reset |
| `/api/auth/change-password` | `POST` | Authenticated password change |
| `/api/accounts/tree` | `GET` | Hierarchical account tree with recursive balances |
| `/api/transactions` | `GET`, `POST` | Journal transactions with splits |
| `/api/reconcile/:id` | `GET`, `POST` | Bank account reconciliation tool |
| `/api/reports/balance-sheet` | `GET` | Balance Sheet financial statement |
| `/api/reports/income-statement`| `GET` | Profit & Loss (P&L) statement |
| `/api/reports/trial-balance` | `GET` | Trial Balance report |
| `/api/seed` | `POST` | Seeds standard accounts & transactions |
| `/api/reset-data` | `POST` | Wipes all accounts and transactions to zero |
| `/api/import/gnucash-sqlite` | `POST` | Imports native GnuCash SQLite databases |
| `/api/export/gnucash-sqlite` | `GET` | Exports database as GnuCash SQLite (`username_yyyyMMddhhmmss.gnucash`) |
