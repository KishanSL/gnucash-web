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
Puppeteer testing suites are configured in `frontend/scripts/`:
- `frontend/scripts/test_ui.mjs` — General UI layout, auth page, desktop user dropdown, and mobile drawer.
- `frontend/scripts/test_register_autocomplete.mjs` — Complete account history autocomplete & autofill verification.
- `frontend/scripts/test_register_enhancements.mjs` — Register 2-line mode, inline blank row, and transaction edit modal.

- **Default Test Credentials**: `kishan` / `Kishan@123`
- **Frontend URL**: `http://localhost:8081`
- **Run the test script**:
  ```bash
  cd frontend && node scripts/test_ui.mjs
  cd frontend && node scripts/test_register_autocomplete.mjs
  ```
- **Screenshots Directory**: `frontend/screenshots/`

### React 18 Event Handling & Puppeteer Testing Gotchas
1. **Controlled Inputs Tracker Reset**:
   React 18's internal `_valueTracker` ignores synthetic input events when typing into controlled number or text inputs. Always reset the tracker when programmatically setting values in Puppeteer:
   ```js
   const tracker = el._valueTracker;
   if (tracker) tracker.setValue('');
   Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set.call(el, val);
   el.dispatchEvent(new Event('input', { bubbles: true }));
   ```
2. **Dropdown Trigger Events**:
   Always bind both `onClick` and `onFocus` (`onClick={() => setIsOpen(true)}`) on dropdown inputs (e.g. AccountAutocomplete, Payee Autocomplete) so clicking an already-focused input re-opens the dropdown.

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
    2. **Currency Preference** (white font in dropdown for high contrast against dark backgrounds)
    3. **Import .gnucash** (opens GnuCash SQLite / XML import modal — *never place on main navbar*)
    4. **Export .gnucash** (downloads database as standard SQLite file formatted `username_yyyyMMddhhmmss.gnucash`)
    5. **Seed Sample Data** (seeds chart of accounts & realistic sample transactions)
    6. **Change Password** (opens credentials modal)
    7. **Reset Entire Database** (warning amber button with `RotateCcw` icon)
    8. **Log Out**
- **Mobile Slide-down Drawer**:
  - Must mirror all actions and preferences available in the desktop user dropdown.

---

## 4. Accounts Tree & Account Register (Ledger) Guidelines

- **Accounts Tree Stage Persistence**:
  - Account tree expansion stage (`hide subaccounts` vs `show all accounts`) is preserved in `localStorage` (`gnucash_accounts_collapsed_keys`) across page refreshes.
- **Double-Click to Open Register**:
  - Double-clicking any account row in `AccountsTree` navigates directly to that account's ledger register.
- **Auto-Scroll to Bottom & Inline Blank Entry Row**:
  - The ledger register automatically scrolls to the end where an inline blank row is ready for fast keyboard-driven data entry.
  - Transactions default to 2 splits (the current account + selectable transfer account).
  - Deposit vs. withdrawal single-number entry enforces mutual exclusivity.
- **Complete Historical Payee Autocomplete**:
  - Autocomplete in the register sources from **100% of the account's historical transactions** (`registerEntries`), not limited to recent paginated transactions.
  - Slices distinct payees sorted chronologically descending (most recent first) with occurrence counts (`Temple 3×`).
  - Selecting a payee automatically autofills description, transfer account (from `all_splits`), notes, memo, and debit/credit amount and direction.
- **Two-Line Ledger Mode**:
  - Line 1: Date, Num/Check #, Description / Payee, Transfer Account, Reconciled state (`n`/`c`/`y`), Amount (Deposit/Withdrawal), Running Balance, and Actions.
  - Line 2: Details / Memo / Notes text visibly displayed beneath the transaction.
- **Transaction Edit Option**:
  - An Edit (pencil icon) button is placed immediately next to the Delete (trash icon) button on each transaction row in both `AccountRegister` and `TransactionsJournal`.
  - Clicking Edit opens `TransactionModal` pre-populated with all splits, accounts, amounts, notes, and memo for full modification via `PUT /api/transactions/:id`.

---

## 5. Database, Accounting & Export Rules

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
5. **GnuCash 5.17+ SQLite File Compatibility**:
   - Export endpoint (`GET /api/export/gnucash-sqlite`) writes a valid SQLite database with filename `username_yyyyMMddhhmmss.gnucash`.
   - The `versions` table must populate standard GnuCash 5.17 version numbers to prevent "The database is from an older version of GnuCash" warnings:
     - `table_name = 'Gnucash'`, `table_version = 5000017` (representing version 5.17)
     - `table_name = 'Gnucash-Resave'`, `table_version = 19920`
     - `table_name = 'transactions'`, `table_version = 4`
     - `table_name = 'splits'`, `table_version = 5`
     - `table_name = 'accounts'`, `table_version = 1`
     - `table_name = 'commodities'`, `table_version = 1`
     - `table_name = 'books'`, `table_version = 1`

---

## 6. Build & Deployment Workflows

Because services run in Podman containers, rebuild and redeploy after code changes:

### Frontend Updates
```bash
cd frontend && npm run build
podman cp dist/. gnucash-frontend:/usr/share/nginx/html/
podman restart gnucash-frontend
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

## 7. Key API Endpoints Reference

| Endpoint | Method | Description |
| :--- | :--- | :--- |
| `/api/health` | `GET` | Health check (`service`, `status`, `version`) |
| `/api/auth/login` | `POST` | User login (returns JWT token and user info) |
| `/api/auth/signup` | `POST` | User registration |
| `/api/auth/reset-password` | `POST` | Admin/recovery password reset |
| `/api/auth/change-password` | `POST` | Authenticated password change |
| `/api/accounts/tree` | `GET` | Hierarchical account tree with recursive balances |
| `/api/accounts/:id/register` | `GET` | Full account ledger register with `all_splits`, notes, memo, running balance |
| `/api/transactions` | `GET`, `POST` | Journal transactions with splits |
| `/api/transactions/:id` | `GET`, `PUT`, `DELETE` | Retrieve, update splits/notes/memo, or delete transaction |
| `/api/reconcile/:id` | `GET`, `POST` | Bank account reconciliation tool |
| `/api/reports/balance-sheet` | `GET` | Balance Sheet financial statement |
| `/api/reports/income-statement`| `GET` | Profit & Loss (P&L) statement |
| `/api/reports/trial-balance` | `GET` | Trial Balance report |
| `/api/seed` | `POST` | Seeds standard accounts & transactions |
| `/api/reset-data` | `POST` | Wipes all accounts and transactions to zero |
| `/api/import/gnucash-sqlite` | `POST` | Imports native GnuCash SQLite databases |
| `/api/export/gnucash-sqlite` | `GET` | Exports database as GnuCash SQLite (`username_yyyyMMddhhmmss.gnucash`) |
