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
   - Valid account types: `bank`, `cash`, `asset`, `credit`, `liability`, `equity`, `income`, `expense`, `receivable`, `payable`, `root`.
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
   - Currency preference selector inside the user profile menu supporting `INR (₹)`, `USD ($)`, `EUR (€)`, `GBP (£)`, `JPY (¥)`, `CAD ($)`, `AUD ($)`, and `CHF (Fr.)`.
   - Formats balances and registers according to the user's active currency preference.

6. **Investments (Stocks & Mutual Funds)**:
   - Stocks and mutual funds are tracked by **investment amount only** (money value). Units, prices, and market valuation are out of scope.
   - Investment balances roll up cleanly into parent asset totals without mixing unit counts and currency amounts.
   - For GnuCash round-trips, the importer preserves split quantity and commodity metadata (ISINs, mnemonics, fraction) as pass-through data, and the exporter writes them back faithfully.

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

---

## REST API Reference

### Authentication & User Management
- Public sign-up is disabled by default (`ALLOW_SIGNUP=false`).
- Password reset is CLI-only to prevent user enumeration and account lockout.
- `POST /api/auth/login` - Authenticate via username or email (constant-time check); returns 30-day JWT.
- `POST /api/auth/change-password` - Update password (requires current password; revokes older sessions).
- `GET /api/auth/me` - Get profile for currently authenticated user.
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

Runs against a separate, automatically created `gnucash_test` database (never the live one). Includes **77 automated tests**: 25 unit tests in `src/` and 52 integration tests in `tests/` (`accounts_tests.rs`, `auth_tests.rs`, `transactions_tests.rs`, `reports_tests.rs`, `import_export_tests.rs`, `tenancy_data_tests.rs`, sharing `tests/common/mod.rs`):
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

### Frontend Build & UI Verification:
```bash
cd frontend
npm run build
node scripts/test_ui.mjs   # read-only smoke check against the deployed app
npm run test:e2e           # full Puppeteer suite in an isolated, throwaway environment
```

### Whole CI pipeline locally:
```bash
./scripts/ci_local.sh      # the same checks as .forgejo/workflows/ci.yml (add --no-e2e to skip Puppeteer)
```
Verifies TypeScript compilation, production asset bundling, and browser UI interaction. `npm run test:e2e` runs against a temporary `gnucash_e2e_test` database, so it never touches live data.
