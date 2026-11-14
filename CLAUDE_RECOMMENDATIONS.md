# GnuCash Web — Security & Optimisation Review

_Reviewed: 2026-11-14 · Scope: `backend/` (Rust/Axum/SQLx), `frontend/` (React/Vite), `docker-compose.yml`, `nginx.conf`, `scripts/` · Commit `7d75244`_

Findings are ordered by severity. Every item cites `file:line` so it can be verified directly. Nothing in this document has been changed in code yet.

| Severity | Count |
| :--- | :--- |
| 🔴 Critical | 3 |
| 🟠 High | 7 |
| 🟡 Medium | 12 |
| 🔵 Low / Hardening | 10 |
| ⚡ Performance / Optimisation | 14 |

---

## 🔴 Critical

### C1. Cross-tenant data takeover through GnuCash import (`ON CONFLICT … SET user_id = EXCLUDED.user_id`)
**Where:** `backend/src/handlers/importer.rs:648-654`, `:686-689`, `:714-719`

The importer uses the GUIDs from the uploaded file as **primary keys** and upserts with `ON CONFLICT (id) DO UPDATE SET … user_id = EXCLUDED.user_id`. If a row with that id already belongs to **another user**, the upsert silently moves it to the importer's account.

- **Accidental trigger:** two users (e.g. a household) both import the same `.gnucash` file. The second import takes over the first user's accounts and transactions. The first user's splits still point at accounts they no longer own.
- **Deliberate trigger:** an attacker who learns another user's account or transaction UUIDs (from a shared export file, a screenshot or logs) builds a GnuCash file with those GUIDs and imports it.

**Fix:**
1. Don't use file GUIDs as primary keys. Give every imported row a fresh `Uuid::new_v4()` (or `Uuid::new_v5(user_namespace, guid)` so ids are per-user and deterministic), and keep the original GUID in a separate `gnc_guid` column, unique per `(user_id, gnc_guid)`.
2. If upsert stays, restrict it to the caller's own rows:
   ```sql
   ON CONFLICT (id) DO UPDATE SET ... WHERE accounts.user_id = EXCLUDED.user_id
   ```
   Then treat "0 rows affected" as a conflict error.
3. Add a regression test: user A imports file F, user B imports F, and A's data must be unchanged.

### C2. Default JWT secret committed to the repository
**Where:** `docker-compose.yml:36`

```yaml
JWT_SECRET: ${JWT_SECRET:-b88caf1e27ae5f4f518317a4a9c4c71b67b274dac5f8028a6c3a741b490d0813}
```

Any deployment without its own `JWT_SECRET` in `.env` uses this public value. The startup check (`main.rs:20`) only checks the length, so the server starts normally. With this secret anyone can:
- forge a JWT for any user id (`auth.rs:97-102`), which gives full read/write access to every tenant, and
- use it as the **admin key** (`X-Admin-Key`) to create users and reset any password (`handlers/auth.rs:24`, `:340`).

The images are also published to Docker Hub (`kishansl/*`), so this default reaches everyone who pulls them.

**Fix:** Remove the fallback (`JWT_SECRET: ${JWT_SECRET:?JWT_SECRET must be set}`). Rotate the secret on every existing deployment. Optionally refuse to start when the secret matches a known-bad list.

### C3. Changing the password does not require the current password
**Where:** `backend/src/handlers/auth.rs:500-512`; client sends it as optional in `frontend/src/components/ChangePasswordModal.tsx:48`

`current_password` is checked **only if it is supplied and non-empty**. Anyone holding a token (XSS, a shared or stolen laptop, a leaked token valid for 30 days) can call `POST /api/auth/change-password` with only `new_password` and lock the real owner out for good.

**Fix:** Make `current_password` required (non-`Option`) and always verify it. The one exception should be an explicit "must change temporary password" flow, gated by a server-side flag set during the admin reset.

---

## 🟠 High

### H1. Any authenticated user can crash the backend with an account-parent cycle (stack overflow)
**Where:** `backend/src/handlers/accounts.rs:264-288` (only blocks `parent_id == id`), recursion at `:429-444`, `:484-500`, and `reports.rs:84-99`, `:255-270`

`update_account` stops an account from being its own parent, but not longer loops (set A→B, then B→A). After that, `calc_total_balance` recurses forever on the next `GET /api/accounts`, `/tree` or report call. **In Rust a stack overflow aborts the whole process**, not just the request, so every tenant is taken offline. `restart: unless-stopped` brings the server back, but the bad data remains and the next request crashes it again.

**Fix:**
- Before updating, walk up the ancestors of the new parent (recursive CTE) and reject the change if `id` appears:
  ```sql
  WITH RECURSIVE anc AS (
    SELECT id, parent_id FROM accounts WHERE id = $new_parent AND user_id = $uid
    UNION ALL
    SELECT a.id, a.parent_id FROM accounts a JOIN anc ON a.id = anc.parent_id
  ) SELECT EXISTS (SELECT 1 FROM anc WHERE id = $self)
  ```
- Make the tree builders iterative or track visited nodes, so data that is already corrupt (e.g. from imports, see `importer.rs:684`) can't crash the server.

### H2. A test-mode environment variable fully bypasses authentication
**Where:** `backend/src/auth.rs:128-134` and `:70-72`

If `GNUCASH_TEST_MODE=1|true` is set in the **production binary** (not just `cfg(test)`), then:
- requests with **no** `Authorization` header are authenticated as user `…0001` (`kishan`), and
- a known, hard-coded JWT secret is used.

One wrong environment variable (copied from a CI config or a debug session) turns off all authentication.

**Fix:** Put this behaviour behind `#[cfg(test)]` or a Cargo feature (`--features test-auth`) so it can't be compiled into release builds. Integration tests in `tests/` can mint a real JWT with the test secret instead.

### H3. Default user with a known password, logged in plaintext
**Where:** `backend/src/db.rs:39-51`

On an empty `users` table the server creates `kishan` / `password123` and logs the password (`info!("Created default user: kishan (password: password123)")`). All seeded or auto-imported data belongs to this account. On a new deployment exposed through Cloudflare Tunnel, this is a ready-made login.

**Fix:** Don't create a default user, or generate a random password and write it only to the `0600` security file (like the reset notice). Never log the password. Require a password change on first login.

### H4. The admin key is the JWT signing secret, and it is compared in non-constant time
**Where:** `backend/src/handlers/auth.rs:23-24`, `:339-340`; `scripts/create_user.sh:37-40`, `scripts/reset_password.sh:95-98`

- **Key reuse:** a leaked admin key also lets an attacker forge tokens, and the other way round. Rotating one forces rotating the other.
- `admin_key == jwt_secret` uses ordinary string comparison. A timing attack over a network is impractical against a 64-hex secret, but this costs nothing to fix.
- The admin endpoints are reachable from the **public** nginx `/api/` location, without the login rate limit.
- The CLI scripts send the secret as a `curl -H` **argument**, so other local users can see it in `ps`.

**Fix:** Add a separate `ADMIN_API_KEY` (or better, a backend CLI subcommand such as `gnucash-backend admin reset-password <user>` that talks to Postgres directly, so no HTTP admin path is exposed). Compare with `subtle::ConstantTimeEq`. Deny `X-Admin-Key` routes in nginx unless the request comes from `127.0.0.1`. Pass headers to curl with `-H @-` or `--config` from stdin.

### H5. Rate-limit bypass by spoofing `CF-Connecting-IP`
**Where:** `frontend/nginx.conf:2-8`

The rate-limit key comes from `$http_cf_connecting_ip`, which **any client** can set. An attacker who sends a random `CF-Connecting-IP` with each request gets a fresh 20 r/m bucket every time, so login brute-forcing is not limited. This applies even through Cloudflare if the origin is reachable any other way. Note that the frontend port is not bound to localhost (see M1).

**Fix:** Trust that header only from Cloudflare's IP ranges, using `ngx_http_realip_module`:
```nginx
set_real_ip_from 173.245.48.0/20;   # …all Cloudflare ranges (or 127.0.0.1 for cloudflared)
real_ip_header CF-Connecting-IP;
limit_req_zone $binary_remote_addr zone=auth_limit:10m rate=20r/m;
```

### H6. The import endpoint wipes all user data by default
**Where:** `backend/src/handlers/importer.rs:507` (`wipe.unwrap_or(true)`)

A destructive operation is the **default**. Any request without `?wipe=false` (a script, a retried request, a CSRF-like action if auth ever moves to cookies) deletes every account and transaction before importing. Together with C1, a failed import can lose data in more ways than expected.

**Fix:** Default to `false` and require an explicit `wipe=true` sent from a confirmed UI action. Consider taking an automatic export snapshot before wiping.

### H7. Imports over 2 MB fail, because Axum's default body limit was not raised
**Where:** `backend/src/lib.rs:87-90`; size check at `importer.rs:539`

Axum 0.7 applies a **2 MB `DefaultBodyLimit`** to `Multipart`. nginx allows 50 MB (`nginx.conf:61`) and the handler checks for 50 MB, but real GnuCash books larger than 2 MB are rejected with a vague multipart error first. Also, the 50 MB check runs only **after** the whole body is in memory.

**Fix:** On the import route only:
```rust
.route("/api/import/gnucash-sqlite", post(import_gnucash_sqlite).layer(DefaultBodyLimit::max(50 * 1024 * 1024)))
```
Keep a small global limit (e.g. 1 MB) for the JSON endpoints.

---

## 🟡 Medium

### M1. The frontend port is published on all interfaces
**Where:** `docker-compose.yml:58` — `"${FRONTEND_PORT:-8081}:80"`

`AGENTS.md` says all ports are bound to `127.0.0.1`, but the frontend is bound to `0.0.0.0`. The app (and through it the whole `/api`, including the admin endpoints) is reachable from the LAN and, on a VPS, from the internet, without going through Cloudflare. **Fix:** `"127.0.0.1:${FRONTEND_PORT:-8081}:80"`.

### M2. Internal error details are returned to clients
**Where:** about 60 places, e.g. `auth.rs:145` (`Invalid or expired token: {e}`), `handlers/auth.rs:73`, `accounts.rs:204`, `transactions.rs:319`, `importer.rs:609`

Raw `sqlx::Error` text (table and column names, constraint names, SQL fragments) and JWT library errors are sent in JSON. **Fix:** Add one `AppError` type that implements `IntoResponse`, logs the detailed error with `tracing::error!`, and returns a generic message plus a request id. This also removes about 500 lines of repeated `map_err` boilerplate (see P13).

### M3. Admin password reset writes plaintext passwords to up to 6 locations, including `/tmp`
**Where:** `backend/src/handlers/auth.rs:221-274`; also echoed in the response at `:451`

The notice is written to `$PASSWORD_RESET_FILE`, `/app/security`, `/app`, `./`, `./security` and `/tmp/gnucash_password_reset.txt`. Even an admin-chosen **permanent** password is written to disk (`:426`). Old notices are never removed, and each one is a long-lived credential.

**Fix:** Write to a single configured location. Don't persist admin-specified permanent passwords; just confirm success. Mark temporary passwords as "must change", expire them (e.g. 24 h), and delete the notice once the password is changed.

### M4. Stateless 30-day JWTs cannot be revoked
**Where:** `backend/src/auth.rs:81-83`

Changing or resetting the password does **not** invalidate existing tokens, so a stolen token stays valid for up to 30 days. Deleted users' tokens also keep working until expiry.

**Fix:** Add `users.token_version` (or `password_changed_at`). Put it in the claims and check it in `AuthUser::from_request_parts` (one indexed lookup per request, or cache it). Bump it on password change or reset, and add a "log out everywhere" option. Also shorten the lifetime (e.g. 12–24 h) and add a refresh token.

### M5. Tokens are stored in `localStorage`
**Where:** `frontend/src/api.ts:24-33`

Any XSS (including through a compromised npm dependency) can read the token and use it from elsewhere for 30 days. **Fix:** Move to an `HttpOnly; Secure; SameSite=Strict` cookie set by the backend, with a CSRF token or `SameSite=Strict` plus an `Origin` check on state-changing routes. At minimum, add a strict CSP (L1) to reduce XSS risk.

### M6. Permissive CORS (`Any` origin, method and header)
**Where:** `backend/src/lib.rs:15-18`

Because auth uses a bearer header this can't be exploited directly today. It becomes a real hole if auth moves to cookies (M5), and it lets any website call the API from the user's browser if a token is ever attached automatically. The SPA is same-origin through nginx, so CORS isn't needed at all. **Fix:** Remove the layer, or allow only the configured public origin.

### M7. The balance tolerance lets unbalanced ledgers through, and extra precision is silently rounded
**Where:** `backend/src/handlers/transactions.rs:247`, `:465`; schema `NUMERIC(14,2)` at `migrations/001_initial_schema.sql:30`; import `importer.rs:438`

- `sum.abs() > 0.01` accepts a difference of **±0.01**, so every transaction can be off by up to a cent and the trial balance drifts.
- Amounts with more than 2 decimal places (`10.005`, or imported `1/3`) are accepted, then rounded **per split** by Postgres, which can create imbalance.
- Imported splits are never checked for balance, so an imported file can contain unbalanced transactions or transactions with only one split.

**Fix:** Reject amounts whose `scale() > 2` (or round each split first, then require `sum == 0` **exactly**). Apply the same checks (≥2 splits, sum = 0, no placeholder accounts) to the importer, or report imbalances back to the user.

### M8. Splits can be marked reconciled directly; `reconciled_state` is not validated
**Where:** `backend/src/handlers/transactions.rs:325-326`, `:506-507`

Clients can send any `reconciled_state`. `"y"` skips the reconcile flow, and values longer than 1 character cause a DB error (which leaks details, see M2). Editing a transaction also deletes and recreates its splits (`:493`), which resets reconcile state and dates unless the client sends them back. **Fix:** Accept only `n` or `c` on create and update (only `/reconcile` may set `y`), and preserve existing reconcile metadata for unchanged splits. Add `CHECK (reconciled_state IN ('n','c','y'))`.

### M9. Unique constraints ignore case; there is a race between check and insert
**Where:** `backend/src/handlers/auth.rs:63-82`; `migrations/002_multi_user.sql:3-4`

The check is `LOWER(username) = LOWER($1)`, but the unique constraint is on raw `username`. Two concurrent signups for `Alice` and `alice` both pass the check. Login (`:137-143`) then finds two matching rows, and `fetch_optional` silently picks one. A username that equals another user's email has the same problem. **Fix:** `CREATE UNIQUE INDEX ON users (LOWER(username)); CREATE UNIQUE INDEX ON users (LOWER(email));`, block `@` in usernames, and map the unique-violation error to `409`.

### M10. Weak password policy; passwords are silently trimmed
**Where:** `backend/src/handlers/auth.rs:39`, `:55`, `:128`, `:467`

The minimum is 6 characters, there's no maximum (bcrypt only uses the first 72 bytes), and `.trim()` changes passwords that intentionally start or end with spaces. **Fix:** Require at least 10–12 characters and reject passwords over 72 bytes (or pre-hash). Don't trim passwords. Optionally check against a common-password list.

### M11. Seed and starter provisioning are not atomic
**Where:** `backend/src/handlers/seed.rs:46-59`; `handlers/auth.rs:109-111`, `:542-653`

`seed_database` commits the wipe, then seeds in separate statements on the pool. If seeding fails, the user ends up with an **empty** book. `provision_starter_accounts` has the same problem, and signup logs the failure but still returns `201`. **Fix:** Pass `&mut Transaction` through `run_seed` and `provision_starter_accounts`, and do the wipe and seed in a single transaction.

### M12. Deleting a parent account silently cascades to its child accounts
**Where:** `migrations/001_initial_schema.sql:6` (`parent_id … ON DELETE CASCADE`); `accounts.rs:369-390`

`delete_account` checks only the target's **own** splits. Children without splits are deleted silently. Children with splits make the cascade fail on `splits … ON DELETE RESTRICT`, which surfaces as a raw 500 error. **Fix:** Reject the delete if the account has children (or offer "move children to parent"), and change the FK to `ON DELETE RESTRICT`.

---

## 🔵 Low / Hardening

| # | Issue | Where | Recommendation |
| :--- | :--- | :--- | :--- |
| L1 | No `Content-Security-Policy`, no `Strict-Transport-Security` | `frontend/nginx.conf:20-23` | `default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'; base-uri 'none'`. Add HSTS when served over HTTPS (Cloudflare). |
| L2 | Only `/api/auth/login` is rate-limited | `nginx.conf:33` | Also limit `signup`, `reset-password`, `change-password` (it verifies bcrypt, see C3), and `import`/`export` (expensive). |
| L3 | Malicious SQLite upload hardening | `importer.rs:321-325` | Open with `OpenFlags::SQLITE_OPEN_READ_ONLY`, set `PRAGMA trusted_schema=OFF`, enable `SQLITE_DBCONFIG_DEFENSIVE`, set `conn.set_limit(...)` and a `progress_handler` timeout so crafted files can't use unbounded CPU or memory. Check that `accounts`/`transactions`/`splits` are real tables, not views. |
| L4 | User-supplied text fields have no length or format validation | `accounts.rs:181,192`, `transactions.rs:311`, `importer.rs:508` | Validate `currency` (ISO-4217, 3 uppercase letters), `account_type` (enum), `description` (≤255), `num` (≤50) and number of splits (e.g. ≤100) **before** they reach the DB. Today they cause raw 500s that leak details. |
| L5 | Negative `limit`/`offset` cause a DB error | `transactions.rs:37-38` | `limit.clamp(1, 500)`, `offset.max(0)`. |
| L6 | Search does not escape `%` / `_` | `transactions.rs:67` | Escape LIKE metacharacters (`ESCAPE '\'`). |
| L7 | `Cargo.lock` is gitignored, but `backend/Dockerfile:6` copies it | `.gitignore:4` | A fresh-clone `docker build` fails, and dependency versions aren't reproducible for a binary. **Commit `Cargo.lock`**, then run `cargo audit` / `cargo deny` in CI. |
| L8 | Unpinned base images | `backend/Dockerfile:2` (`rust:latest`), `frontend/Dockerfile:16` (`nginx:alpine`) | Pin versions (`rust:1.xx-bookworm`, `nginx:1.xx-alpine`) or digests. Run nginx as non-root (`nginxinc/nginx-unprivileged`). Drop `curl` from the runtime image (use a tiny health binary or `--health-cmd` from compose). |
| L9 | Default Postgres password `gnucash_secret` | `docker-compose.yml:10,32`, `.env.example:3` | Use `${POSTGRES_PASSWORD:?required}`. Fix the healthcheck, which hard-codes `-U gnucash -d gnucash` (`:19`). |
| L10 | Admin scripts build JSON by string interpolation | `scripts/create_user.sh:40`, `reset_password.sh:88` | A `"` or `\` in a password breaks or injects JSON. Build it with `jq -n --arg`. Passwords given as CLI arguments end up in shell history and `ps`, so read them with `read -s` instead. |

Other small items:
- `console.log` of full transaction payloads in production (`AccountRegister.tsx:266,303,309`). Remove them, or drop them in the build with `esbuild.drop: ['console']`.
- `/api/health` reveals the `allow_signup` setting and version (`lib.rs:100-111`). This is acceptable, but keep it minimal.
- Temporary passwords use 64 bits from UUID v4 hex (`handlers/auth.rs:390-392`). That's acceptable, but `rand::distributions::Alphanumeric` with 20 characters is clearer.

---

## ⚡ Performance & Code Optimisation

### Backend

**P1. bcrypt runs on the async runtime threads.** `bcrypt::hash` and `verify` at cost 12 take about 200–300 ms of CPU each (`handlers/auth.rs:84,159,396,504,514`, `db.rs:40`). A few concurrent logins can block all Tokio worker threads, and every other request stalls. This is also a cheap DoS through the unthrottled endpoints. Wrap these calls in `tokio::task::spawn_blocking`.

**P2. The exporter does synchronous SQLite I/O on the runtime, with one autocommit per row.** `exporter.rs:113-445` runs `rusqlite` inline in an async handler, and each `INSERT` is its own implicit transaction (an fsync per row). For a book with about 20k splits that's tens of seconds and blocks a worker thread throughout. Fix:
```rust
tokio::task::spawn_blocking(move || {
    let mut conn = Connection::open(&path)?;
    conn.pragma_update(None, "journal_mode", "OFF")?;
    conn.pragma_update(None, "synchronous", "OFF")?;
    let tx = conn.transaction()?;
    { let mut stmt = tx.prepare_cached("INSERT INTO splits ...")?; for s in &splits { stmt.execute(params![...])?; } }
    tx.commit()?;
    std::fs::read(&path)
})
```
This usually makes export 50–100× faster.

**P3. The importer inserts one row per round-trip.** `importer.rs:635-783` runs one `INSERT` per account, transaction and split, so a 50 MB file means hundreds of thousands of sequential round-trips inside one long transaction. Batch inserts with `UNNEST`:
```sql
INSERT INTO splits (id, transaction_id, account_id, amount, memo, reconciled_state)
SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::uuid[], $4::numeric[], $5::text[], $6::text[])
```
Use chunks of about 5–10k rows, or `COPY … FROM STDIN` through `PgCopyIn`. Also sort accounts topologically once instead of the 10-pass loop (`:635`), which also silently flattens hierarchies deeper than 10 levels.

**P4. Swallowed error inside a Postgres transaction.** `importer.rs:702` uses `.ok()` on an insert within `tx`. If it fails, Postgres puts the transaction into the *aborted* state, every later statement fails with "current transaction is aborted", and the error reported to the user is misleading. Either propagate the error or use a `SAVEPOINT`.

**P5. Redundant binds.** `importer.rs:726-729` binds 4 extra parameters to a statement with 5 placeholders. Remove them; at best they're dead code, and depending on the driver they can cause a protocol error.

**P6. Recursive balance roll-up is O(n × depth) and recomputed for every row.** `accounts.rs:446-449` calls `calc_total_balance` for each account, and each call walks its whole subtree again. `reports.rs` repeats the same pattern. Compute totals once with a post-order traversal (memoised `HashMap<Uuid, Decimal>`), or in SQL with a recursive CTE. Move the shared tree and roll-up code into one module; it's currently duplicated in `accounts.rs` (×2) and `reports.rs` (×2).

**P7. Duplicate endpoints and duplicate work on every refresh.** `list_accounts` and `get_account_tree` (`accounts.rs:19-91`) run the same two queries. The frontend calls **both**, plus `fetchTransactions` and the register, after every mutation (`App.tsx:120-122`, `:186-198`). Return the tree only and flatten it on the client, or return `{ accounts, tree }` from one call.

**P8. The account register is unbounded and clones every split for every row.** `transactions.rs:599-711`:
- It loads **all** splits for the account, then all sibling splits, and `splits_by_tx.get(..).cloned()` copies each transaction's split list for every register row (`:666`). The JSON response grows roughly as rows × splits per transaction.
- Compute the running balance in SQL with `SUM(s.amount) OVER (ORDER BY t.post_date, t.enter_date, s.id)` and add keyset pagination (`?before=<post_date,id>&limit=500`), loading earlier history on scroll. The frontend autocomplete requirement ("100% of history") can be met with a small dedicated endpoint, `GET /api/accounts/:id/payees` (`SELECT description, COUNT(*), MAX(post_date) … GROUP BY description`), instead of sending the full ledger.
- Use `.remove()` instead of `.cloned()` where each tx id appears only once.

**P9. `list_transactions` uses `SELECT DISTINCT … LEFT JOIN splits`.** `transactions.rs:41-45`. DISTINCT over every column plus a join is costly and forces a sort. Use `WHERE EXISTS (SELECT 1 FROM splits s WHERE s.transaction_id = t.id AND s.account_id = $n)` only when `account_id` is given, and drop the join otherwise.

**P10. Missing and suboptimal indexes.** Add these in a new migration:
```sql
CREATE INDEX IF NOT EXISTS idx_tx_user_date      ON transactions (user_id, post_date DESC, enter_date DESC);
CREATE INDEX IF NOT EXISTS idx_accounts_user_code ON accounts (user_id, code, name);
CREATE INDEX IF NOT EXISTS idx_splits_acct_rec   ON splits (account_id, reconciled_state);
CREATE UNIQUE INDEX IF NOT EXISTS uq_users_username_lower ON users (LOWER(username));
CREATE UNIQUE INDEX IF NOT EXISTS uq_users_email_lower    ON users (LOWER(email));
-- optional, for ILIKE search:
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX IF NOT EXISTS idx_tx_desc_trgm ON transactions USING gin (description gin_trgm_ops);
```
The login query `LOWER(username) = … OR LOWER(email) = …` currently scans the whole `users` table.

**P11. Silent `unwrap_or_default()` on financial queries.** Balance queries at `accounts.rs:48,84,125`, `reports.rs:71,246` and `reconcile.rs:64,76` turn DB errors into **zero balances**. A transient DB failure then shows a wrong balance sheet with no error. Propagate the error instead.

**P12. Migrations run as raw SQL on every start, with no version tracking.** `db.rs:26-31`. Switch to `sqlx::migrate!()`, which tracks versions, wraps each migration in a transaction and refuses to run modified migrations. Also make `accounts.user_id` / `transactions.user_id` `NOT NULL` once the orphan backfill (`db.rs:55-69`) has run, so a missed filter can't create rows that belong to no tenant. Consider adding `user_id` to `splits`, or enabling Postgres Row-Level Security as a backstop for the multi-tenant rule.

**P13. Error-handling boilerplate.** Each handler repeats `map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({...}))))` many times (about 60 occurrences). An `AppError` enum with `impl From<sqlx::Error>` and `impl IntoResponse` would make each of these a `?`, cut a large share of handler code, and fix M2 in the same change.

**P14. Cache JWT keys.** `get_jwt_secret()` reads the environment, and `EncodingKey`/`DecodingKey` are rebuilt on every request (`auth.rs:93`, `:98`). Build them once in a `OnceLock` (or in app state) at startup. Also pin `Validation` to `Algorithm::HS256` explicitly.

### Frontend

- **Register rendering:** `AccountRegister.tsx:62` recomputes `filteredEntries` on every render without `useMemo`, and `:509` / `:842` render every row (each with nested `all_splits` maps). For ledgers with thousands of entries, use `useMemo` plus list virtualisation (`@tanstack/react-virtual` or `react-window`).
- **Partial refresh after mutations:** after creating or editing a transaction, refetch only the affected account's register and the account tree, not the transaction journal and both account endpoints (`App.tsx:186-198`). Or adopt TanStack Query for caching and targeted invalidation.
- **Bundle:** `lucide-react` is tree-shaken only through named imports; check that no `import * as Icons` exists. Lazy-load the report views and modals with `React.lazy` to shrink the initial bundle.

---

## Suggested order of work

1. **Now (exploitable):** C1, C2 (and rotate secrets), C3, H1, H2, M1.
2. **Next:** H3–H7, M2–M4, L7 (commit `Cargo.lock`).
3. **Integrity:** M7, M8, M9, M11, M12, P11, P12.
4. **Performance:** P1, P2, P3, P8, P10, then the rest.
5. **Hardening:** M5, M6, L1–L10.

For each fix, add a regression test in `backend/tests/api_tests.rs`. The most valuable ones are the cross-tenant import (C1), the parent-cycle crash (H1), and change-password without the current password (C3).
