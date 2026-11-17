# Codebase Quality Check

- **Date:** 2026-11-17
- **Branch:** `refactor/codebase-review-and-cleanup` (at `089d42c`, plus uncommitted changes, including the untracked `backend/src/handlers/importer/`, `frontend/src/components/register/`, `frontend/src/components/transaction_modal/`, `frontend/scripts/test_fund_account_decimals.mjs` and `scripts/cleanup_test_data.sh`)
- **Score:** **85 / 100** (up from 81)

It's solid where it counts most: the money and accounting logic, tenant isolation and security hardening are all done carefully. The backend tests are reliable and isolated, CI and linting are strict, and the importer, account register and transaction modal are split into focused modules. Amounts are back to fixed 2 decimals (ACC-3), with a backend test guarding it. What holds the score back now is that the Puppeteer tests write into the live database, plus a few remaining long functions.

## Notes and corrections

- **Earlier advice in this report was wrong.** Previous versions said to use each account's `commodity_fraction` for money amounts. That contradicted ACC-3 ("split amounts have at most 2 decimal places") and INV-5 (fraction is pass-through data only). The change has been reverted, and amounts use `toFixed(2)` again.
- **Correction to the last version.** It said "154 fund accounts in the live data". In fact 153 of those belonged to leftover test users. `kishan` had none until `test_fund_account_decimals.mjs` created one.
- **Some leftover test data came from the reviewer.** The first review ran `cargo test` against the live database, which added 42 test users and 9 fixture transactions in `kishan`. Later reviews ran `test_register_enhancements` against the live backend, which probably added two "Vendor …" transactions.
- **The live database was cleaned on 2026-11-17** with `scripts/cleanup_test_data.sh --apply --no-backup`, at the owner's request with no backup. It removed 1,301 test users (2,378 accounts, 753 transactions) and the test residue inside `kishan` (329 accounts, 423 transactions). What remains is 1 user (`kishan`) with the 37 seed accounts and 15 seed transactions. Splits sum to 0.00 and there are no orphan splits.

## What was run

| Check | Result |
|---|---|
| `cargo clippy --all-targets -- -D warnings` | Passes. |
| `cargo clippy --release -- -D warnings` | Passes (production build, without the `test-utils` feature). |
| `npm run lint` (`--max-warnings 0`) | Passes, with 0 warnings. |
| `npm run typecheck` | Passes. |
| `npm run build` | Passes. |
| `cargo test` (default, parallel), 3 runs | 77 of 77 passed every time (25 unit, 52 integration). |
| Puppeteer tests | Not run this round, because they write into the live database (issue 1). |

## Scorecard

| Area | Score | Notes |
|---|---|---|
| Correctness and domain modelling | 8/10 | Amounts use `Decimal` and the totals are checked in the backend. Balance, placeholder and cycle rules are enforced. Amounts are fixed at 2 decimals end to end (ACC-3), and commodity fraction is pass-through only (INV-5). A requirements doc with IDs is used throughout. |
| Security | 8.5/10 | Queries use bound parameters, `user_id` filters are consistent, and database errors are hidden from API responses (SEC-13). Login is constant-time and tokens can be revoked. Ports bind to localhost only, CORS is restricted to configured origins, and test helpers are kept out of production builds. Remaining deduction: the JWT lasts 30 days and is stored in localStorage. |
| Backend structure | 8/10 | The importer is split into `parser`, `validator`, `persister` and `models`. The persister's longest function is 122 lines, and queries read into typed row structs. Remaining: `parse_gnucash_sqlite` is about 284 lines and `parse_gnucash_xml` about 255. No query is checked at compile time. |
| Frontend structure | 7.5/10 | `AccountRegister` is split into a 267-line shell plus `RegisterRow`, `RegisterInlineEntry` and `useAccountRegister`. `TransactionModal` is split into a 411-line shell plus `useTransactionModal` and `TransactionSplitRow`. Errors are typed as `unknown` and go through `getErrorMessage`. Remaining: `App.tsx` is 563 lines with about 20 `useState` calls and no data layer. |
| Database and migrations | 8/10 | Migrations are tracked with `sqlx::migrate!`. |
| Testing | 7.5/10 | Backend coverage is real and meaningful (cross-tenant isolation, import edge cases, rollback, and now `test_fund_account_transaction_fixed_2_decimals` for ACC-3). Backend tests use their own users and an automatically created `_test` database. Deduction: the Puppeteer tests run against the live app and database and leave data behind (issue 1). There are no frontend unit tests. |
| Tooling, CI and docs | 8.5/10 | Forgejo CI runs clippy with warnings as errors, the tests against Postgres 16, typecheck, ESLint with zero warnings allowed, and the build. `requirements.md`, `AGENTS.md` and the README are good, and commits follow a consistent format. |

## Open issues, in order of payoff

1. **The Puppeteer tests write into the live database.**
   - `frontend/scripts/test_fund_account_decimals.mjs` runs `psql` against the live `gnucash` database. It inserts an "Aditya Birla Mutual Fund" account into `kishan` if no fund account exists, then records a real 500.50 "Fund SIP …" transaction on every run.
   - `frontend/scripts/test_register_enhancements.mjs` records a "Vendor …" transaction on every run.

   Give the Puppeteer tests their own environment: a second backend pointed at `gnucash_test` with a dedicated test user, or at least delete what each test creates when it finishes. The fund test should also create its fixture through the API instead of `psql` with hardcoded `-U gnucash -d gnucash`.
2. **Bug in the fund test.** `test_fund_account_decimals.mjs` looks up *any* account with fraction 1000 but never uses that ID. It then searches the screen for the hard-coded name "Aditya Birla Mutual Fund", so it fails whenever the fund account it found has a different name. Use the account it looked up, or the one it created.
3. **Commit the refactor.** About 3,700 lines are uncommitted, including the untracked folders and scripts listed at the top. CI can't check any of it until it's committed and pushed.
4. **Split up the remaining long pieces.**
   - Backend: `parse_gnucash_sqlite` (about 284 lines) and `parse_gnucash_xml` (about 255 lines) in `backend/src/handlers/importer/parser.rs`.
   - Frontend: `frontend/src/App.tsx` is 563 lines with about 20 `useState` calls and no data layer.
5. **Check the CI runner.** Confirm that the Forgejo runner supports the `services:` block and the `ubuntu-latest` label (not verified).
6. **Frontend unit tests.** Add tests for the money arithmetic in `useAccountRegister` and `useTransactionModal`, so ACC-3 is also checked on the frontend without needing a browser.
7. **Token storage.** The JWT lasts 30 days and is stored in localStorage. A shorter lifetime, or an httpOnly cookie, would reduce the impact of an XSS bug.

Isolating the Puppeteer tests (issues 1 and 2) and committing should put this at about 88.
