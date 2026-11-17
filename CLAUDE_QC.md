# Codebase Quality Check

- **Date:** 2026-11-17
- **Branch:** `refactor/codebase-review-and-cleanup` (after `9e398b9`: SEC-14/SEC-15, strict CSP, database-error fixes)
- **Score:** **91 / 100** (up from 87)
- **Reviewer note:** most changes since the previous review were made by the same reviewer (Claude), so this is a self-review. An independent review is recommended for the last 12 commits.

The core is in good shape:
- Money, balances and double-entry rules are exact and enforced on both the frontend and the backend.
- Tenant isolation and security hardening are solid.
- Tests never touch live data, and the large files have been split into focused modules.

What keeps it from the mid-90s is mostly operational: CI has never actually run, because no runner is registered.

## What was run

| Check | Result |
|---|---|
| `cargo clippy --all-targets -- -D warnings` | Passes. |
| `cargo clippy --release -- -D warnings` | Passes (production build, without the `test-utils` feature). |
| `cargo test` | 88 of 88 pass (36 unit, 52 integration), against a separate auto-created `gnucash_test` database. |
| `npm run typecheck` / `npm run lint` (`--max-warnings 0`) | Both pass. |
| `npm test` | 22 of 22 pass (money arithmetic, Node's built-in test runner). |
| `npm audit` | 0 vulnerabilities. |
| `npm run test:e2e` (Puppeteer, isolated) | 12 of 12 suites pass against a throwaway `gnucash_e2e_test` database. Ports 8092 and 8093 are bound to `127.0.0.1` only. |
| Read-only Puppeteer smoke checks against the deployed app | Pass. The live data was unchanged (1 user, 37 accounts, 15 transactions). |
| Strict CSP check on the deployed app | `script-src 'self'`. Logging in, visiting every tab and opening the user menu produces 0 CSP violations. |

## Scorecard

| Area | Score | Notes |
|---|---|---|
| Correctness and domain modelling | 9.5/10 | Amounts use `Decimal` on the backend and integer cents on the frontend (`src/lib/money.ts`). The balance check matches exactly what is submitted (ACC-2), amounts are fixed at 2 decimals (ACC-3), and reconcile totals are exact. Zero balances never show as `-0`. Database failures return a 500 instead of a misleading zero balance or "not found". |
| Security | 9.5/10 | Queries use bound parameters and are scoped by `user_id` (SEC-10). Database errors are hidden from responses (SEC-13), login is constant-time, and tokens are revoked on password change (SEC-5). All ports bind to localhost (SEC-11), and the e2e environment does too. CORS is restricted to configured origins, and `npm audit` is clean. The token is sent as a Bearer header (SEC-14), protected on the web by a strict `script-src 'self'` CSP (SEC-15). |
| Backend structure | 8.5/10 | `accounts`, `transactions`, `exporter` and `importer` are modules with shared validation and pure helpers. The longest handler is about 130 lines. Queries read into typed row structs. SQL is checked at runtime only, by decision (see "Decided, not issues"). |
| Frontend structure | 8.5/10 | `App.tsx` is 332 lines and composes hooks in `src/app/`. The register, transaction modal, auth view, accounts tree and navbar are split into focused components and hooks. The largest components left are `RegisterInlineEntry.tsx` (478 lines) and `ReconcileModal.tsx` (459). |
| Database and migrations | 8/10 | Migrations are tracked with `sqlx::migrate!`, and the migration test runs the real `003` file against a throwaway database. |
| Testing | 9/10 | Backend integration tests are split by area and use their own users. Pure validation, split-sync and tree helpers have unit tests, and the frontend money logic has unit tests. The Puppeteer suites run in an isolated environment, and scripts that change data refuse to run against the live app. Gap: no frontend component tests, and the Puppeteer suites aren't in CI (issue 4). |
| Tooling, CI and docs | 7/10 | The Forgejo workflow covers clippy (warnings as errors), the backend tests against Postgres 16, typecheck, ESLint, unit tests and the build. **No runner is registered, so CI has never run** (issue 1). `requirements.md`, `AGENTS.md` and the README are current. The backend shuts down gracefully, with a 0.2 s restart instead of a 10 s kill. |

## Open issues, in order of payoff

1. **CI has never run.** Forgejo Actions is enabled and the workflow triggers on `refactor/**`, but no runner is registered for the repository or the user, and `actions/tasks` shows 0 runs ever. Install and register a `forgejo-runner` that can run containers, so the `services: postgres` block works, and map the `ubuntu-latest` label. Until then, every CI check has only been run locally.
2. **Puppeteer suites and the UI in CI.** `npm run test:e2e` needs `podman` and the local Postgres container, so it can't run in the current CI workflow. Once a runner exists, consider a CI job that starts Postgres as a service and runs the same suites. There are also no React component tests; UI behaviour is covered only by the Puppeteer suites.
3. **Remaining larger components.** `frontend/src/components/register/RegisterInlineEntry.tsx` (478 lines) and `frontend/src/components/ReconcileModal.tsx` (459) could be split into a hook plus presentational parts, like the rest of the UI. This is low priority.

## Decided, not issues

- **Token storage** (SEC-14, decided 2026-11-17): the JWT stays in `localStorage` and is sent as an `Authorization: Bearer` header, because the native Kotlin Android app authenticates the same way. Cookie-only auth would break it. XSS risk on the web is handled by the strict CSP (SEC-15).
- **Compile-time SQL checks** (`sqlx::query!`): considered and declined by the owner on 2026-11-17. There are 99 queries, 7 of them dynamic, and the integration tests already run them against a real database. Revisit if the schema starts changing often.
- **Login rate limiting:** not covered by `requirements.md`, and the app is bound to localhost. Add it there first if it becomes a requirement.

Getting CI running (issue 1) would put this at about 93. Adding CI coverage for the UI (issue 2) would take it into the mid-90s.
