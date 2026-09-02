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

## Ports & Deployment

| Service | Port | Endpoint / Status |
| :--- | :--- | :--- |
| **Frontend UI** | `8081` | `http://localhost:8081` |
| **Rust Backend** | `8082` | `http://localhost:8082` (`/api/...`) |
| **PostgreSQL DB** | `5432` | `postgres://gnucash:gnucash_secret@127.0.0.1:5432/gnucash` |

---

## REST API Reference

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
