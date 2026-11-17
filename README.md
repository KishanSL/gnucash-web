# GnuCash Web (Full-Stack GnuCash Clone)

A modern, high-performance web clone of **GnuCash** built with **Rust (Axum + SQLx)**, **PostgreSQL**, and **React + TypeScript + Tailwind CSS**.

> **Requirements:** [`requirements.md`](requirements.md) is the source of truth for expected behaviour. If this README disagrees with it, `requirements.md` wins.

---

## Architecture Overview

```
                      +-----------------------------+
                      |       Web Browser           |
                      |   React 18 + TypeScript     |
                      |   Port: 8081 (localhost)    |
                      +--------------+--------------+
                                     |
                                     | HTTP / REST
                                     v
                      +-----------------------------+
                      |       Rust Backend          |
                      |    Axum + SQLx Engine       |
                      |    Port: 8082 (localhost)    |
                      +--------------+--------------+
                                     |
                                     | TCP / SQL
                                     v
                      +-----------------------------+
                      |      PostgreSQL 16          |
                      |   Double-Entry Ledger DB    |
                      |   Port: 5432 (localhost)    |
                      +-----------------------------+
```

---

## Key Features & Accounting Logic

1. **Strict Double-Entry Bookkeeping**:
   - Every transaction requires between 2 and 100 splits.
   - Enforces exact zero balance: $\sum \text{Splits} = 0.00$ ($\text{Total Debits} = \text{Total Credits}$) with no tolerance.
   - Rejects postings to placeholder category accounts.
   - Supports transaction-level notes and split-level memos.

2. **Hierarchical Chart of Accounts (Tree Structure)**:
   - Nested structure: Assets, Liabilities, Equity, Income, Expenses.
   - Valid account types: `bank`, `cash`, `asset`, `credit`, `liability`, `equity`, `income`, `expense`, `receivable`, `payable`. `root` is rejected on create and update. Imported `STOCK`/`MUTUAL` accounts are stored as `asset` (INV-7).
   - A child account's type must be compatible with its parent's (ACC-12).
   - Computes both **direct balance** and **recursive subtree total balance** with depth cycle guards.

3. **Account Register (Ledger View)**:
   - Primary day-to-day GnuCash register view.
   - Chronological running balances calculated dynamically.
   - Reconciled indicators: `n` (unreconciled), `c` (cleared), `y` (reconciled).
   - Two-line mode: line 1 shows date, num, description, transfer account, reconciled state, amount, and running balance; line 2 shows notes/memo.
   - Full historical payee autocomplete with occurrence ranking.

4. **Interactive Bank & Credit Card Reconciliation**:
   - Statement date and statement ending balance input.
   - Starting balance dynamically computed from all reconciled (`y`) splits.
   - Split checkoff workflow: separates Funds In (Deposits) and Funds Out (Payments).
   - Real-time calculation of Starting Balance, Cleared Balance, and Target Difference.
   - One-click commit to permanently mark splits as reconciled (`y`) with reconciliation timestamp.

5. **Multi-Currency System & Indian Numbering**:
   - **Default Currency**: Indian Rupee (`INR` - `₹`) with Indian numbering format (`en-IN`: `₹1,23,456.78`).
   - Currency preference selector inside the user profile menu supporting `INR (₹)`, `USD ($)`, `EUR (€)`, `GBP (£)`, `JPY (¥)`, `CAD (C$)`, `AUD (A$)`, `AED (د.إ)`, `SGD (S$)`, and `CHF`.
   - Amounts are stored and entered with at most 2 decimal places (ACC-3); the display preference does not convert between currencies.
   - Formats balances and registers according to the user's active currency preference.

6. **Investments (Stocks & Mutual Funds)**:
   - Stocks and mutual funds are tracked by **investment amount only** (money value). Units, prices, and market valuation are out of scope.
   - Investment balances roll up cleanly into parent asset totals without mixing unit counts and currency amounts.
   - For GnuCash round-trips, the importer preserves split quantity and commodity metadata (ISINs, mnemonics, fraction) as pass-through data, and the exporter writes them back (INV-5).
   - Known limitation (INV-6): when an investment split is edited in the web app, its quantity is set equal to its amount, so the next export carries that value as the unit count.

7. **Native GnuCash Import & Export (.gnucash SQLite & XML)**:
   - **Import**: Direct multipart upload via `POST /api/import/gnucash-sqlite` accepting both GnuCash SQLite and XML (plain or gzip-compressed) up to 50 MB.
     - Supports **wipe** mode (clean overwrite) or **merge** mode (upsert by GUID).
     - Full transaction rollback on unbalanced files, scheduled-transaction template skipping, and cycle breaking.
   - **Export**: One-click download from the user menu via `GET /api/export/gnucash-sqlite`.
     - Generates standard `<username>_<timestamp>.gnucash` SQLite books compatible with desktop GnuCash 5.17+ without upgrade warnings.

8. **Financial Reports**:
   - **Balance Sheet**: Assets vs. Liabilities + Equity (+ Net Income) as of a date.
   - **Income Statement (P&L)**: Revenues vs. Operating Expenses over a date range.
   - **Trial Balance**: Demonstrates double-entry consistency ($\text{Total Debits} = \text{Total Credits}$).

9. **Clean Desktop & Mobile UI**:
   - Desktop header contains only: New Account, New Entry, and the User Menu.
   - User dropdown menu provides: Profile details, Currency preference, Import .gnucash, Export .gnucash, Seed Sample Data, Change Password, Reset Entire Database, and Log Out.
   - Mobile responsive drawer with full access to actions.

---

## Docker Compose Deployment (1-Command Launch)

Deploy the full stack (PostgreSQL + Rust Backend + React Frontend/Nginx) with a single command:

```bash
docker compose up -d
```

### Services & Port Exposure:

| Service | Container Name | Localhost Binding | Container Port | Description |
| :--- | :--- | :--- | :--- | :--- |
| **Frontend UI** | `gnucash-frontend` | `127.0.0.1:8081` | `80` | React 18 SPA served via Nginx (reverse-proxies `/api/` to backend) |
| **Rust Backend** | `gnucash-backend` | `127.0.0.1:8082` | `8082` | Axum REST API server |
| **PostgreSQL DB** | `gnucash-postgres` | `127.0.0.1:5432` | `5432` | PostgreSQL 16 database |

> [!IMPORTANT]
> All container ports bind strictly to `127.0.0.1` (localhost only). External exposure must only be routed through a secure reverse proxy or tunnel pointed to `http://127.0.0.1:8081`.

### Initial Administrator Setup:
On first launch with an empty database:
- The initial admin account username is read from `INITIAL_ADMIN_USER` (default `kishan`).
- Password is read from `INITIAL_ADMIN_PASSWORD`.
- If `INITIAL_ADMIN_PASSWORD` is unset or blank, the backend automatically generates a secure random password and writes it to `./security/initial_admin_credentials.txt` (mounted at `/app/security`).

### Automatic Seed & Import:
- On first startup against an uninitialized database, the backend checks `./data` (mounted at `/app/data`, configured by `GNUCASH_SEED_DIR`) for `.gnucash` files.
- If a `.gnucash` file is found, it is automatically imported.
- If no file is present, standard Chart of Accounts and realistic sample transactions are seeded.

### Managing Containers:

```bash
# View live container logs
docker compose logs -f

# Check container health & status
docker compose ps

# Stop all services
docker compose down

# Stop and wipe database volume (clean reset)
docker compose down -v
```

### Configuration (environment variables)

Set these in `.env` (see `.env.example`); `docker compose` passes them to the containers.

| Variable | Used by | Default | Purpose |
| :--- | :--- | :--- | :--- |
| `JWT_SECRET` | backend | **required** | Signs session tokens. Must be at least 32 characters, or the backend refuses to start (`openssl rand -hex 32`). |
| `ADMIN_API_KEY` | backend, CLI scripts | falls back to `JWT_SECRET` | Admin key for `create_user.sh` and `reset_password.sh` (SEC-2). |
| `ALLOW_SIGNUP` | backend | `false` | Allow public sign-up without the admin key. |
| `INITIAL_ADMIN_USER` | backend | `kishan` | Username of the first admin, created when the database has no users. |
| `INITIAL_ADMIN_PASSWORD` | backend | empty (random) | First admin's password; if empty, a random one is written to `$SECURITY_DIR/initial_admin_credentials.txt`. |
| `GNUCASH_SEED_DIR` | backend | `/app/data` | Folder checked for a `.gnucash` file to auto-import on first start. |
| `SECURITY_DIR` | backend | `/app/security` | Where initial admin credentials are written. |
| `PASSWORD_RESET_FILE` | backend | `/app/security/password_reset.txt` | Where CLI password-reset notices (temporary passwords) are written. |
| `DATABASE_URL` | backend | set by compose | PostgreSQL connection string. |
| `CORS_ALLOWED_ORIGINS` | backend | `http://localhost:8081`, `http://127.0.0.1:8081` and the same for `:5173` | Comma-separated allowed origins (`*` for any). The bundled frontend calls the API same-origin through Nginx, so this rarely needs changing. |
| `BIND_ADDR` / `PORT` | backend | `0.0.0.0` / `8082` | Listen address and port (inside the container; the host binding is controlled by compose). |
| `FRONTEND_BIND` / `FRONTEND_PORT` | compose | `127.0.0.1` / `8081` | Host address and port for the web UI. |
| `BACKEND_PORT` | compose | `8082` | Host port for the API (always bound to `127.0.0.1`). |
| `POSTGRES_USER` / `POSTGRES_PASSWORD` / `POSTGRES_DB` / `POSTGRES_PORT` | compose | `gnucash` / `gnucash_secret` / `gnucash` / `5432` | Database credentials and host port (bound to `127.0.0.1`). Change the password for any real deployment. |

### Security Headers

The frontend Nginx sends `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, a strict `Referrer-Policy` and `Permissions-Policy`, and a Content-Security-Policy with `script-src 'self'` (no inline or external scripts, SEC-15).

---

## Local Development

```bash
# Database only (from the repository root)
docker compose up -d postgres

# Backend (http://127.0.0.1:8082)
cd backend
JWT_SECRET=$(openssl rand -hex 32) cargo run

# Frontend dev server (http://localhost:8081, proxies /api to 127.0.0.1:8082)
cd frontend
npm install
npm run dev
```

Notes:
- **Data:** without `DATABASE_URL`, the backend connects to `postgres://gnucash:gnucash_secret@127.0.0.1:5432/gnucash`, which is the same database the deployed app uses. For experiments, point `DATABASE_URL` at a separate database, or use `npm run test:e2e`, which runs everything against a throwaway database.
- **Ports:** `cargo run` listens on 8082 and `npm run dev` on 8081, the same as the deployed `gnucash-backend` and `gnucash-frontend` containers. Stop those first, or use other ports (`PORT=…` for the backend; set `API_PROXY_TARGET` to point the Vite proxy at it).

To deploy changes into the running containers without rebuilding images, follow **AGENTS.md section 6** (build, `podman cp` / `docker cp` into the container, restart).

---

## REST API Reference

### Authentication & User Management
- **Every endpoint except login, health and the admin-key endpoints requires `Authorization: Bearer <JWT>`** (SEC-14). The web app keeps the token in `localStorage`; native clients (e.g. the Android app) send the same header. Cookie-only auth is deliberately not used.
- Public sign-up is disabled by default (`ALLOW_SIGNUP=false`).
- Password reset is CLI-only to prevent user enumeration and account lockout.
- `POST /api/auth/login` - Authenticate via username or email (constant-time check); returns a 30-day JWT and the user.
- `POST /api/auth/change-password` - Update password (requires current password; revokes all older tokens).
- `GET /api/auth/me` - Get profile for currently authenticated user.
- `POST /api/auth/signup` - Create a user. Requires the admin key (`X-Admin-Key` header, used by `create_user.sh`) unless `ALLOW_SIGNUP=true`.
- `POST /api/auth/reset-password` - Admin-key only (`X-Admin-Key`, used by `reset_password.sh`); anonymous callers get `403` before any database query.
- **Admin CLI Scripts**:
  ```bash
  # Create new user via admin key:
  ./scripts/create_user.sh <username> <email> <password>

  # Reset password via server CLI:
  ./scripts/reset_password.sh <username_or_email> [optional_new_password]
  ```

### Health & Data Management
- `GET /api/health` - Health check status.
- `POST /api/seed` - Seed standard Chart of Accounts & sample transactions into current user space.
- `POST /api/reset-data` - Permanently wipe all accounts and transactions for the authenticated user.

### Accounts
- `GET /api/accounts` - List all accounts with direct and recursive balances.
- `GET /api/accounts/tree` - Hierarchical nested tree of accounts.
- `GET /api/accounts/:id` - Get specific account details.
- `POST /api/accounts` - Create new account.
- `PUT /api/accounts/:id` - Update account details (prevents hierarchy cycles).
- `DELETE /api/accounts/:id` - Delete account (requires 0 splits and no children).
- `GET /api/accounts/:id/register` - Account register with chronological running balance.

### Transactions
- `GET /api/transactions` - Filterable transaction journal.
- `GET /api/transactions/:id` - Transaction details with all splits.
- `POST /api/transactions` - Create multi-split transaction (enforces exact double-entry balance).
- `PUT /api/transactions/:id` - Update transaction and splits (preserves reconcile state when account/amount are unchanged).
- `DELETE /api/transactions/:id` - Delete transaction and its splits.

### Reconciliation
- `GET /api/reconcile/:id` - Fetch unreconciled deposits & withdrawals up to statement date.
- `POST /api/reconcile/:id` - Mark cleared splits as reconciled (`y`).

### Financial Reports
- `GET /api/reports/balance-sheet?date=YYYY-MM-DD` - Balance Sheet (Assets = Liabilities + Equity + Net Income).
- `GET /api/reports/income-statement?from_date=YYYY-MM-DD&to_date=YYYY-MM-DD` - Income Statement (P&L).
- `GET /api/reports/trial-balance?date=YYYY-MM-DD` - Trial Balance (Debits = Credits).

### GnuCash Import & Export
- `POST /api/import/gnucash-sqlite` - Multipart upload for GnuCash SQLite & XML books (`wipe=true|false`).
- `GET /api/export/gnucash-sqlite` - Download database as desktop-compatible GnuCash SQLite file.

---

## Running the Automated Test Suite

### Backend Test Suite (Rust):
```bash
cd backend
cargo test
```

Runs against a separate, automatically created `gnucash_test` database (never the live one; the database name must end in `_test`). Includes **88 automated tests**: 36 unit tests in `src/` and 52 integration tests in `tests/` (`accounts_tests.rs`, `auth_tests.rs`, `transactions_tests.rs`, `reports_tests.rs`, `import_export_tests.rs`, `tenancy_data_tests.rs`, sharing `tests/common/mod.rs`):
- Health check verification
- Double-entry imbalance rejection ($400 Bad Request) with exact zero tolerance
- Minimum 2 splits and maximum 100 splits validation
- Prevention of postings to placeholder accounts
- Account hierarchy cycle prevention and recursive depth bounds
- Account register chronological running balance verification
- Interactive reconciliation starting balance and commit workflow
- Balance Sheet accounting equation ($\text{Assets} = \text{Liabilities} + \text{Equity}$)
- Trial Balance total debits equal total credits check
- GnuCash SQLite & compressed gzip XML import parsing and validation
- Cross-tenant data isolation on import and queries
- Re-import without wipe replacing stale splits
- Template transaction exclusion
- Investment accounts displaying investment currency amounts rather than share units
- GnuCash SQLite export schema versioning, commodity namespaces, and quantity preservation
- Secure initial admin credentials and default password rejection
- JWT invalidation upon password change and non-enumerating login

### Frontend Checks & UI Verification:
```bash
cd frontend
npm run typecheck          # TypeScript (strict)
npm run lint               # ESLint, zero warnings allowed
npm test                   # 22 unit tests (money arithmetic), Node's built-in test runner
npm run build              # production bundle
npm run test:e2e           # all 12 Puppeteer suites in an isolated, throwaway environment
node scripts/test_ui.mjs   # read-only smoke check against the deployed app
```

`npm run test:e2e` starts a temporary backend on a fresh `gnucash_e2e_test` database and a preview build, both bound to `127.0.0.1`, then tears everything down; it never touches live data. Puppeteer scripts that change data refuse to run against the live app.

### Whole CI pipeline locally:
```bash
./scripts/ci_local.sh      # the same checks as .forgejo/workflows/ci.yml (add --no-e2e to skip Puppeteer)
```

---

## Continuous Integration

`.forgejo/workflows/ci.yml` runs on every push to `master`, `main`, `refactor/**`, `feat/**` and `fix/**`, and on pull requests to `master`/`main`, using a self-hosted Forgejo runner:

| Job | What it runs |
| :--- | :--- |
| Backend Lint & Test | `cargo clippy --all-targets -D warnings` and `cargo test` against a PostgreSQL 16 service |
| Frontend Typecheck & Lint | `npm ci`, typecheck, ESLint, unit tests, production build |
| Puppeteer UI Suites | all 12 suites via `npm run test:e2e` against a PostgreSQL service (screenshots are uploaded if a job fails) |
