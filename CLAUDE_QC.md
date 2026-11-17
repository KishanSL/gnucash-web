# Codebase Quality Check

- **Date:** 2026-11-17
- **Branch:** `refactor/codebase-review-and-cleanup` (at `52a967b`)
- **Score:** **73 / 100**

It's solid where it counts most: the money and accounting logic, tenant isolation and security hardening are all done carefully. The points are lost on test reliability, oversized code units, and missing CI and linting.

## What was run

| Check | Result |
|---|---|
| `cargo clippy --all-targets` | No warnings. |
| `npm run typecheck` | Passes. |
| `cargo test` (default, parallel) | 76 tests (25 unit, 51 integration). Intermittent: 3 of 8 runs had a failure (see issue 1). |

## Scorecard

| Area | Score | Notes |
|---|---|---|
| Correctness and domain modelling | 8/10 | Amounts use `Decimal` and the totals are checked in the backend. Balance, placeholder and cycle rules are enforced. A requirements doc with IDs is used throughout. |
| Security | 8.5/10 | Queries use bound parameters, `user_id` filters are consistent, and database errors are hidden from API responses (SEC-13). Login is constant-time and tokens can be revoked. Ports bind to localhost only, and CORS is restricted to configured origins. Remaining deduction: the JWT lasts 30 days and is stored in localStorage. |
| Backend structure | 6/10 | The layout is clean (`handlers/`, `models`, `auth`). But `save_gnucash_data_to_db` is one function of about 863 lines, and `importer.rs` is 1,810 lines. 9-field query tuples are copied between handlers instead of using shared row types, and no query is checked at compile time. |
| Frontend structure | 5/10 | `AccountRegister.tsx` is 1,299 lines and `TransactionModal.tsx` is 828. `App.tsx` has 20 `useState` calls and no data layer. Money is handled with `parseFloat`/`toFixed(2)`, which ignores each commodity's decimal places. There are many `catch (err: any)` blocks and no ESLint. |
| Database and migrations | 7/10 | Migrations are tracked with `sqlx::migrate!`. The reconciled-state cleanup statements are still in migration `003` and are also repeated on every boot in `db.rs`, which is redundant now that the constraint blocks bad data. |
| Testing | 6.5/10 | Coverage is real and meaningful (cross-tenant isolation, import edge cases, rollback), and the tests run against a separate `gnucash_test` database. But tests share state and fail intermittently (see issues 1–3). |
| Tooling, CI and docs | 6.5/10 | `requirements.md`, `AGENTS.md` and the README are good, and commits follow a consistent format. `package.json` has `typecheck` and end-to-end scripts. There's no CI pipeline, no ESLint and no frontend unit tests. |

## Open issues, in order of payoff

1. **Tests share state and fail intermittently when run in parallel.** There are two causes:
   - `test_issue_30_reconciled_state_migration_idempotent` (`backend/tests/api_tests.rs:4151`) removes the global `chk_splits_reconciled_state` constraint and inserts invalid rows. Meanwhile, every other test's database setup tries to add that constraint back, which fails with error `23514` "check constraint … is violated".
   - `test_auth_change_password` (`backend/tests/api_tests.rs:752`) changes the password of the shared `kishan` test user. That signs out any other test using the same user mid-run, so they get 401 instead of 200.

   Fix: give each test its own user, and run the constraint test against its own temporary table (or mark it to run serially).
2. **Nothing in the repo creates the `gnucash_test` database.** It isn't in `docker-compose.yml` or the scripts, so on a fresh clone the tests fail until someone creates it by hand.
3. **The safeguard on the test database address is weak.** `_test` is only appended when `DATABASE_URL` ends exactly in `/gnucash`. Any other form, such as `…/gnucash?sslmode=disable` or a different database name, is used unchanged, so the tests would run against that real database. Require `TEST_DATABASE_URL`, or refuse to run unless the database name ends in `_test`.
4. **Add CI** that runs clippy, `tsc`, ESLint and the tests against a throwaway Postgres. Add ESLint to the frontend.
5. **Break up the oversized units.** Split the importer into parse, validate and persist steps. Split `AccountRegister` into a row component, an inline-entry component and a hook.
6. **Duplicated code.** The test database address logic is copy-pasted three times: `backend/tests/api_tests.rs:15`, `backend/tests/api_tests.rs:4154` and `backend/src/handlers/seed.rs:446`. Move it into one shared helper. Drop the redundant reconciled-state cleanup that runs on every boot in `db.rs`.
7. **Seed failures are ignored.** The baseline seed in `get_test_pool` discards its errors (`let _ =`), so a broken seed shows up later as confusing test failures instead of a clear error.
8. **Frontend money handling.** Use each account's `commodity_fraction` instead of a hardcoded `toFixed(2)`.

Fixing issues 1, 2 and 4 would put this around 78. Breaking up the importer and `AccountRegister` is what gets it into the 80s.
