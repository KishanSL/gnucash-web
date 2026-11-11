# GnuCash Web (Full-Stack GnuCash Clone)

A modern, high-performance web clone of **GnuCash** built with **Rust (Axum + SQLx)**, **PostgreSQL**, and **React + TypeScript + Tailwind CSS**.

## Architecture Overview

```
                      +-----------------------------+
                      |       Web Browser           |
                      |   React 18 + TypeScript     |
                      |   Port: 8081                |
                      +--------------+--------------+
                                     |
                                     | HTTP / REST
                                     v
                      +-----------------------------+
                      |       Rust Backend          |
                      |    Axum + SQLx Engine       |
                      |    Port: 8082               |
                      +--------------+--------------+
                                     |
                                     | TCP / SQL
                                     v
                      +-----------------------------+
                      |      PostgreSQL 16          |
                      |   Double-Entry Ledger DB    |
                      |   Port: 5432                |
                      +-----------------------------+
```

---

## Key Features & Accounting Logic

1. **Strict Double-Entry Bookkeeping**:
   - Every transaction requires $\ge 2$ splits.
   - Enforces the fundamental accounting rule: $\sum \text{Splits} = 0.00$ ($\text{Total Debits} = \text{Total Credits}$).
   - Rejects posting to placeholder category accounts.

2. **Hierarchical Chart of Accounts (Tree Structure)**:
   - Nested structure: Assets, Liabilities, Equity, Income, Expenses.
   - Supports Bank accounts, Cash, Credit cards, Accounts Receivable, and Payables.
   - Computes both **direct balance** and **recursive subtree total balance**.

3. **Account Register (Ledger View)**:
   - Primary day-to-day GnuCash register view.
   - Chronological running balances calculated dynamically.
   - Multi-split transaction expansion (inspect all legs of a transaction inline).
   - Reconciled indicators: `n` (unreconciled), `c` (cleared), `y` (reconciled).

4. **Interactive Bank & Credit Card Reconciliation**:
   - Statement date and statement ending balance input.
   - Split checkoff workflow: separates Funds In (Deposits) and Funds Out (Payments).
   - Real-time calculation of Starting Balance, Cleared Balance, and Target Difference.
   - One-click commit to permanently mark splits as reconciled (`y`).

5. **Multi-Currency System**:
   - **Default Currency**: Indian Rupee (`INR` - `₹`) with Indian numbering format (`en-IN`: `₹1,23,456.78`).
   - Global currency selector in the navigation bar supporting `INR (₹)`, `USD ($)`, `EUR (€)`, `GBP (£)`, `JPY (¥)`, `CAD ($)`, `AUD ($)`, and `CHF (Fr.)`.
   - Accounts can define distinct currencies, and transactions/registers format with the active preference.

6. **Native GnuCash SQLite `.gnucash` Import**:
   - One-click modal import in the UI (`Import GnuCash` button in top navigation).
   - Direct multipart file upload to `POST /api/import/gnucash-sqlite`.
   - Parses official GnuCash SQLite schemas:
     - GUID translation (32-character hexadecimal to RFC 4122 UUIDs).
     - Rational number division (`value_num / value_denom` converted precisely via `rust_decimal::Decimal`).
     - Reconciliation state mapping (`reconcile_state`: `'n'`, `'c'`, `'y'`).
     - Hierarchical parent-child account dependency resolution in topological order.
   - Toggle to either overwrite (clean wipe) or merge into existing books.

7. **Financial Reports**:
   - **Balance Sheet**: Assets vs. Liabilities + Equity (+ Net Income) as of a date.
   - **Income Statement (P&L)**: Revenues vs. Operating Expenses over a date range.
   - **Trial Balance**: Demonstrates double-entry consistency ($\text{Total Debits} = \text{Total Credits}$).

---

## Docker Compose Deployment (1-Command Launch)

Deploy the entire full-stack application (PostgreSQL + Rust Backend + React Frontend/Nginx) with a single command:

```bash
docker compose up -d
```

### Services Started:

| Service | Container Name | Port Exposure | Description |
| :--- | :--- | :--- | :--- |
| **Frontend UI** | `gnucash-frontend` | `127.0.0.1:8081` (Localhost only) | React 18 SPA served via Nginx reverse proxy |
| **Rust Backend** | `gnucash-backend` | `127.0.0.1:8082` (Localhost only) | Axum REST API server (accessible via Nginx `/api/`) |
| **PostgreSQL DB** | `gnucash-postgres` | `127.0.0.1:5432` (Localhost only) | PostgreSQL 16 database |

> [!NOTE]
> All services bind strictly to `127.0.0.1` (localhost only). If exposing via Cloudflare Tunnel or reverse proxy, point your tunnel to `http://127.0.0.1:8081` without exposing raw container ports to public interfaces or `0.0.0.0`.
>
> File imports are restricted strictly to direct browser multipart file uploads (`POST /api/import/gnucash-sqlite`). Server-side path imports have been completely removed for security.

### Automatic GnuCash File Import:
- On startup, if the database is uninitialized, the backend automatically scans `~/codebase` (mounted into the container) for any `.gnucash` files (such as `KishanSenLaskar-2026-27.gnucash`) and auto-imports them on first boot!
- If no file is detected, it seeds default standard accounts.
- You can also import any `.gnucash` file at any time using the **Import GnuCash** button in the web UI at `http://localhost:8081`.

### Managing Containers:

```bash
# View container logs
docker compose logs -f

# Check status
docker compose ps

# Stop all services
docker compose down

# Stop and wipe database volume
docker compose down -v
```

---

## Ports & Access

| Service | Host Port | Internal Docker Port | Access URL |
| :--- | :--- | :--- | :--- |
| **Web UI & API Proxy** | `8081` | `80` | `http://localhost:8081` |
| **Rust Backend API** | *None* | `8082` | Proxied via `http://localhost:8081/api/...` |
| **PostgreSQL Database** | *None* | `5432` | Internal to container network |

---

## REST API Reference

### Authentication & Admin CLI
- Public user registration and public password reset are disabled to prevent brute force, account lockout, and username enumeration attacks over the network.
- **Admin User Creation**:
  ```bash
  ./scripts/create_user.sh <username> <email> <password>
  ```
- **Admin Password Reset**:
  ```bash
  # Generate random temporary password written to server file:
  ./scripts/reset_password.sh <username_or_email>

  # Or set permanent password directly from server CLI:
  ./scripts/reset_password.sh <username_or_email> 'NewPassword@123'
  ```
- `POST /api/auth/login` - Authenticate and obtain 30-day JWT token (protected by constant-time verification)
- `POST /api/auth/change-password` - Update password in authenticated user space
- `GET /api/auth/me` - Get current authenticated user profile

### Health & Seed
- `GET /api/health` - Service health status
- `POST /api/seed` - Pre-populate standard Chart of Accounts & realistic sample transactions

### Accounts
- `GET /api/accounts` - List all accounts with direct and recursive balances
- `GET /api/accounts/tree` - Hierarchical nested tree of accounts
- `GET /api/accounts/:id` - Get specific account details
- `POST /api/accounts` - Create new account (with parent category & placeholder options)
- `PUT /api/accounts/:id` - Update account details
- `DELETE /api/accounts/:id` - Delete empty account
- `GET /api/accounts/:id/register` - Account register with running balance

### Transactions
- `GET /api/transactions` - Filterable transaction journal
- `GET /api/transactions/:id` - Transaction details with all splits
- `POST /api/transactions` - Create multi-split transaction with double-entry balance check
- `PUT /api/transactions/:id` - Update transaction and splits
- `DELETE /api/transactions/:id` - Delete transaction

### Reconciliation
- `GET /api/reconcile/:account_id` - Unreconciled deposits & withdrawals up to statement date
- `POST /api/reconcile/:account_id` - Mark cleared splits as reconciled (`y`)

### Reports
- `GET /api/reports/balance-sheet?date=YYYY-MM-DD` - Balance Sheet report
- `GET /api/reports/income-statement?from_date=YYYY-MM-DD&to_date=YYYY-MM-DD` - P&L report
- `GET /api/reports/trial-balance?date=YYYY-MM-DD` - Trial Balance report

### GnuCash SQLite Import
- `POST /api/import/gnucash-sqlite` - Multipart form upload (`file`, `wipe_existing`, `default_currency`)

---

## Running the Automated Test Suite

```bash
cd backend
cargo test -- --test-threads=1
```

Includes 11 automated integration and business-logic tests:
- Health check verification
- Double-entry imbalance rejection ($400 Bad Request)
- Minimum 2 splits validation
- Prevention of postings to placeholder accounts
- Multi-split transaction balance accuracy
- Account register running balance verification
- Reconciliation workflow
- Balance Sheet accounting equation ($\text{Assets} = \text{Liabilities} + \text{Equity}$)
- Trial Balance debits equal credits check
- GnuCash SQLite file import parsing, account tree construction, and rational amount calculation
