# GnuCash Web: Requirements

**Status:** Source of truth for expected behaviour. Last updated 2026-11-15.

This file defines what GnuCash Web must do. When it disagrees with the code, README.md, AGENTS.md
or a Forgejo issue, this file wins: either the code has a bug, or the requirement here must be
changed first. Audits (`forgejo-audit`) and fix verification (`forgejo-verify`) judge behaviour
against this file.

- **must**: required; a violation is a bug.
- **should**: expected unless there is a documented reason not to.
- **out of scope**: deliberately not supported for now; don't build it without changing this file.

Requirement IDs (e.g. `INV-2`) are stable. Reference them in issues, commits and tests.

---

## 1. Product scope

- **SCOPE-1** GnuCash Web is a multi-user, browser-based double-entry bookkeeping application for
  personal finances that can exchange data with desktop GnuCash 5.17+.
- **SCOPE-2** Default currency is Indian Rupee (`INR`, `₹`), formatted in Indian numbering
  (`₹1,23,456.78`).
- **SCOPE-3** Out of scope for now: market prices and valuation, unit/share tracking for
  investments (see §5), scheduled transactions, budgets, business features (invoices, customers,
  vendors), multi-currency conversion.

## 2. Users, tenancy and security

### Accounts and sign-in
- **SEC-1** Public sign-up is disabled by default (`ALLOW_SIGNUP=false`). New users are created by
  an administrator through `scripts/create_user.sh`, which authenticates with the admin key.
- **SEC-2** The admin key is `ADMIN_API_KEY`. If that is unset, empty or blank, the admin key is
  `JWT_SECRET`.
- **SEC-3** Usernames are at least 3 characters. Passwords are 6–72 characters.
- **SEC-4** Login accepts username or email (case-insensitive). A failed login returns the same
  `401 {"error": "Invalid username or password"}` whether or not the user exists, with constant-time
  password checking (no user enumeration through response or timing).
- **SEC-5** Sessions are JWTs valid for 30 days. Changing a password or an admin password reset
  must invalidate every token issued before it.
- **SEC-6** Password reset is CLI-only (`scripts/reset_password.sh`, admin key). The public
  `POST /api/auth/reset-password` returns `403` to anonymous callers before touching the database.
- **SEC-7** Changing a password requires the current password.

### Initial administrator
- **SEC-8** No password is ever hard-coded for a deployable (release) build. When the database has
  no users, the backend creates the initial admin:
  - username from `INITIAL_ADMIN_USER` (default `kishan`);
  - password from `INITIAL_ADMIN_PASSWORD`; an empty or blank value counts as unset;
  - if unset, a random password is generated and written to
    `$SECURITY_DIR/initial_admin_credentials.txt` (default `/app/security`).
- **SEC-9** The fixed test password `Kishan@123` may only be used in test mode (`cfg!(test)` or
  `GNUCASH_TEST_MODE`), never in a normal release run.

### Tenancy and exposure
- **SEC-10** Every query on accounts, transactions and splits is scoped to the authenticated user.
  No user can read, modify, or infer the existence of another user's data.
- **SEC-11** All container ports bind to `127.0.0.1` by default (frontend 8081, backend 8082,
  PostgreSQL 5432). External access goes only through a tunnel or reverse proxy.
- **SEC-12** Client-supplied identifiers are never used as primary keys for new rows.
- **SEC-13** Error responses must not echo raw database errors.

## 3. Accounting core

### Double entry
- **ACC-1** Every transaction has 2–100 splits.
- **ACC-2** Every transaction balances exactly: the sum of its split amounts is `0.00`, with no
  tolerance. This holds for API create, API update and import.
- **ACC-3** Split amounts have at most 2 decimal places. Positive = debit, negative = credit.
- **ACC-4** Splits cannot be posted to placeholder accounts.
- **ACC-5** Several splits of one transaction may use the same account.

### Balances
- **ACC-6** An account's balance is the sum of its split amounts (money). Its total balance adds
  the balances of all descendants. Every balance shown anywhere (account list, tree, register,
  reports) is a money amount; units are never added to money.
- **ACC-7** Display sign: asset-type (`asset`, `bank`, `cash`, `receivable`) and `expense` accounts
  show the raw balance; all other types show it negated, so normal balances appear positive.
- **ACC-8** The balance sheet must balance (Assets = Liabilities + Equity + net income), and the
  trial balance must show total debits = total credits.

### Account types and hierarchy
- **ACC-9** Valid account types: `bank`, `cash`, `asset`, `credit`, `liability`, `equity`,
  `income`, `expense`, `receivable`, `payable`, `root`. Any other type is rejected (`400`) on
  create and update.
- **ACC-10** Accounts form a tree. An account cannot be its own parent or the descendant of itself,
  and cycles must never be stored. Setting `parent_id` to `null` moves an account to the top level.
- **ACC-11** Account name 1–255 characters. Currency/commodity code 1–64 characters.

## 4. Transactions, register and reconciliation

### Transactions
- **TXN-1** Description 1–255 characters. Num/check reference at most 50 characters.
- **TXN-2** Update requests use three states per optional field: absent = keep, `null` = clear,
  value = set. This applies to `num`, `notes` (transactions) and `code`, `description`,
  `parent_id` (accounts).
- **TXN-3** Each transaction has optional notes. Each split has an optional memo.

### Reconciled state
- **REC-1** A split's reconciled state is one of `n` (not reconciled), `c` (cleared), `y`
  (reconciled), stored lowercase. Input is case-insensitive. The database rejects other values.
- **REC-2** A `y` split carries the date it was reconciled.
- **REC-3** Editing a transaction keeps each split's reconciled state and reconcile date when that
  split's account and amount are unchanged (e.g. fixing a description).
- **REC-4** If a split's account or amount changes, the split resets to `n` and its reconcile date
  is cleared.
- **REC-5** Reconciliation (`/api/reconcile/:id`) takes a statement date. Its starting balance is
  the sum of the account's `y` splits, and committing marks the selected splits `y`.

### Register (ledger)
- **REG-1** Double-clicking an account in the tree opens its register.
- **REG-2** The register shows a running balance, scrolls to the end, and ends with an inline
  blank row for keyboard entry. New entries default to 2 splits (this account + a transfer
  account), and deposit and withdrawal are mutually exclusive.
- **REG-3** Two-line mode: line 1 shows date, num, description, transfer account, reconciled
  state, amount and running balance; line 2 shows notes/memo.
- **REG-4** Payee autocomplete uses all of the account's historical transactions (not just one
  page), most recent first, with occurrence counts. Choosing a payee fills description, transfer
  account, notes, memo, amount and direction.
- **REG-5** Each transaction row in the register and journal has Edit (pencil) next to Delete
  (trash). Edit opens the transaction dialog with all splits pre-filled.

## 5. Investments (stocks and mutual funds)

Stocks and mutual funds are tracked by **investment amount only**: the money paid into them,
recorded at transaction value. Units, prices and market value are not tracked.

- **INV-1** An investment account's balance is the sum of its split amounts in money (e.g. buying
  10 units for ₹1,500 gives a balance of ₹1,500). It is never a unit count.
- **INV-2** Investment balances roll up into parent totals and reports like any other money
  balance (e.g. Checking −₹1,500 + Fund ₹1,500 under Assets gives an Assets total of ₹0).
- **INV-3** The app never shows units, prices, market value or unrealised gains for an investment
  account. Users don't enter units.
- **INV-4** Importing a GnuCash book with stock or mutual-fund accounts must succeed, including
  commodity mnemonics up to 64 characters (ISINs like `INF209K01VA3`, tickers like
  `HDFCBANK.NS`).
- **INV-5** For GnuCash round-trips, the importer keeps each split's quantity and each account's
  commodity namespace and fraction as pass-through data. The exporter writes them back. This data
  must not affect any balance shown in the app.
- **INV-6** Accepted limitation: when an investment split is edited in the web app, its quantity
  is set equal to its amount, so the next export carries that value as the unit count. (Forgejo
  #20, closed as not planned.)
- **INV-7** Imported `STOCK` and `MUTUAL` accounts are stored as type `asset` (current behaviour;
  see open question Q2).

## 6. GnuCash import

- **IMP-1** Import is only by authenticated multipart upload to `POST /api/import/gnucash-sqlite`.
  It accepts GnuCash SQLite and XML (plain or gzip). There is no server-path import or file listing.
- **IMP-2** Upload and decompressed size are each limited to 50 MB.
- **IMP-3** Two modes:
  - **wipe**: replace all of the user's accounts and transactions with the file's contents;
  - **merge**: upsert accounts and transactions by GUID. Each imported transaction's splits are
    replaced by the file's splits, so no stale or duplicate splits remain.
- **IMP-4** The whole import is one database transaction. A transaction in the file that doesn't
  balance exactly (ACC-2) rejects the import with `400` and nothing is written.
- **IMP-5** The book's root account is dropped and its children become top-level accounts.
  Scheduled-transaction templates (template root, its accounts and their transactions) are never
  imported, in both SQLite and XML.
- **IMP-6** Account parent cycles in the file are broken by making the affected account
  top-level; they are never stored.
- **IMP-7** Transaction notes are imported from the `notes` slot (SQLite `slots`, XML `trn:slots`).
- **IMP-8** GnuCash reconcile states other than `n`/`c`/`y` (e.g. `f`, `v`) are imported as `n`.
- **IMP-9** Import uses batched statements, not one round trip per row, so books of 50k+ splits
  finish well within the 300 s proxy timeout.

## 7. GnuCash export

- **EXP-1** `GET /api/export/gnucash-sqlite` downloads a GnuCash SQLite file named
  `<username>_<yyyyMMddhhmmss>.gnucash` that opens in GnuCash 5.17+ without an
  "older version" warning.
- **EXP-2** The `versions` table contains: `Gnucash` = 5000017, `Gnucash-Resave` = 19920,
  `transactions` = 4, `splits` = 5, `accounts` = 1, `commodities` = 1, `books` = 1.
- **EXP-3** Post dates are written at GnuCash neutral time (`YYYYMMDD105900`).
- **EXP-4** Transaction notes are written as a `notes` string slot.
- **EXP-5** Commodities are written with their stored namespace and fraction (INV-5). If those are
  missing, 3-letter alphabetic codes are `CURRENCY` and other codes are `FUND`.

## 8. Data management

- **DATA-1** Reset Entire Database (`POST /api/reset-data`) permanently deletes all of the user's
  accounts and transactions, leaving 0 of each. Other users are unaffected.
- **DATA-2** Seed Sample Data (`POST /api/seed`) replaces the user's data with the standard
  37-account chart of accounts and realistic sample transactions.
- **DATA-3** Schema migrations are idempotent and run on every backend start. A migration must not
  fail on existing valid data.

## 9. User interface

- **UI-1** The desktop header contains only: New Account, New Entry, and the user menu.
- **UI-2** The user menu contains, in this order: profile (username, email); currency preference
  (white text); Import .gnucash; Export .gnucash; Seed Sample Data; Change Password; Reset Entire
  Database (amber, `RotateCcw` icon); Log Out. Import is never on the main navbar.
- **UI-3** The mobile drawer offers every action and preference in the desktop user menu.
- **UI-4** Auth page: two columns on desktop (1440×900 and wider), with branding and a 2×2
  feature grid next to the login card. On mobile (375×667) it collapses to a single card.
- **UI-5** The accounts tree remembers its expand/collapse state across reloads (`localStorage`
  key `gnucash_accounts_collapsed_keys`).
- **UI-6** Currency preference changes formatting only; amounts are not converted (see Q4).
- **UI-7** Every UI change is checked with the Puppeteer scripts in `frontend/scripts/` before it
  is considered done.

## 10. Open questions

These items are ambiguous or undecided. Until one is resolved here, don't change the current
behaviour described above.

- **Q1 Selling investments.** INV-1 defines the balance as the sum of transaction values. Selling
  units for more or less than they cost therefore leaves a balance that is neither the remaining
  cost nor zero, unless a separate gain/loss split is recorded (GnuCash's way). Should a sale
  reduce the investment by its **cost** (with the difference booked to an income/expense account),
  or by the **sale proceeds**? Do we require a gain/loss split?
- **Q2 Investment account type.** Imported `STOCK`/`MUTUAL` accounts become `asset` (INV-7), so an
  export writes them back as `ASSET` and desktop GnuCash no longer treats them as investments.
  Should the app add `stock` and `mutual` types (asset-like for ACC-7) and keep them on
  round-trip?
- **Q3 Investment account currency.** For an imported fund, the account's `currency` field holds
  the fund's mnemonic (e.g. `INF209K01VA3`), while its amounts are in the transaction currency
  (INR). Which currency should the UI show for such an account?
- **Q4 Mixed currencies.** Amounts are stored in each transaction's currency and summed without
  conversion. Is a book limited to a single currency, or do reports need conversion (currently out
  of scope, SCOPE-3)?
- **Q5 Exported unit counts.** Is INV-6 acceptable for books you re-open in desktop GnuCash, or
  should the export write `quantity = amount` for every investment split consistently?
