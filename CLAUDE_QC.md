# Codebase Quality Check

- **Date:** 2026-11-17
- **Branch:** `refactor/codebase-review-and-cleanup` (at `e17040b`)
- **Score:** **68 / 100**

It's solid where it counts most: the money and accounting logic, tenant isolation and security hardening are all done carefully. The points are lost on maintainability and engineering hygiene, and the test setup is the weakest part.

## What was run

| Check | Result |
|---|---|
| `cargo clippy --all-targets` | The source code has no warnings. The test file has 19 warnings, all the same minor kind (a needless borrow). |
| `npx tsc --noEmit` (strict mode) | Passes. |
| `cargo test -- --test-threads=1` (with `DATABASE_URL` set) | All 76 pass (25 unit, 51 integration). |
| `cargo test` (default) | Fails. The 3 database-backed unit tests in `seed.rs` panic because `DATABASE_URL` isn't set. With it set, the integration tests still fail when run in parallel, with Postgres reporting deadlocks (`40P01`). |

## Scorecard

| Area | Score | Notes |
|---|---|---|
| Correctness and domain modelling | 8/10 | Amounts use `Decimal` and the totals are checked in the backend. Balance, placeholder and cycle rules are enforced. A requirements doc with IDs is used throughout. |
| Security | 8/10 | Queries use bound parameters, `user_id` filters are consistent, and database errors are hidden from API responses (SEC-13). Login is constant-time and tokens can be revoked. Ports bind to localhost only. Deductions: CORS allows any origin, a fixed bcrypt hash is used as a fallback if hashing fails (`backend/src/db.rs`), and the JWT lasts 30 days and is stored in localStorage. |
| Backend structure | 6/10 | The layout is clean (`handlers/`, `models`, `auth`). But `save_gnucash_data_to_db` is one function of about 863 lines, and `importer.rs` is 1,810 lines. 9-field query tuples are copied between handlers instead of using shared row types, and no query is checked at compile time. |
| Frontend structure | 5/10 | `AccountRegister.tsx` is 1,299 lines and `TransactionModal.tsx` is 828. `App.tsx` has 20 `useState` calls and no data layer. Money is handled with `parseFloat`/`toFixed(2)`, which ignores each commodity's decimal places. There are many `catch (err: any)` blocks and no ESLint. |
| Database and migrations | 5/10 | There's no record of which migrations have run, so all three re-run on every boot (`backend/src/db.rs`). That limits you to idempotent SQL, and it's what deadlocks when several of them start at once. |
| Testing | 6/10 | Coverage is real and meaningful (cross-tenant isolation, import edge cases, rollback). But see issue 1 below. |
| Tooling, CI and docs | 6/10 | `requirements.md`, `AGENTS.md` and the README are good, and commits follow a consistent format. There's no CI, no frontend linter and no frontend unit tests. The Puppeteer scripts aren't wired into `npm` scripts. |

## Top issues to fix, in order of payoff

1. **The tests write into your real data.** By default they use the same database as the running app. Requests without a token act as user `00000000-0000-0000-0000-000000000001`, which is `kishan` (`backend/src/auth.rs:139`), so every test run adds test accounts and transactions to that account. Point the tests at a separate database and give each test its own user (or wrap it in a transaction).
2. **Migrations.** Switch to `sqlx::migrate!` so each migration runs once and is recorded. That also fixes the parallel test deadlock.
3. **Break up the oversized units.** Split the importer into parse, validate and persist steps. Split `AccountRegister` into a row component, an inline-entry component and a hook.
4. **Add CI** that runs clippy, `tsc`, ESLint and the tests against a throwaway Postgres.
5. **Tidy-ups:** restrict CORS to the frontend's origin; fail loudly if bcrypt hashing fails instead of storing the fixed hash; use each account's `commodity_fraction` instead of a hardcoded `toFixed(2)`.

Fixing items 1 to 4 would likely bring this into the low 80s.
