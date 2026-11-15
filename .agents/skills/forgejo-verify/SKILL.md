---
name: forgejo-verify
description: >-
  Read open issues from the GnuCash Web Forgejo bug tracker, check the current code and the diff
  since each issue was filed or last verified, and mark each one verified-fixed (closed),
  partially-fixed or not-fixed with an evidence comment. Always includes issues a developer has
  labelled `status/solved`, open or closed. Use when the user asks to verify fixes, check which
  Forgejo or audit issues are solved, or triage the bug tracker against the code.
argument-hint: "[all] [#issue ...] [recheck] [dry-run]"
---

# Forgejo Fix Verification

For each open issue, decide from the code at HEAD whether it is solved. Then record the verdict in
Forgejo: a label, a comment with evidence, and the issue closed or reopened to match.

Arguments (`$ARGUMENTS`):
- *(none)* checks open issues labelled `audit`, plus every issue (open or closed) labelled
  `status/solved`.
- `all` checks every open issue, including ones filed by hand, plus `status/solved` issues.
- `#12 #15` checks only these issues, open or closed.
- `recheck` also re-examines closed `status/verified-fixed` issues to catch regressions.
- `dry-run` prints verdicts and changes nothing in Forgejo.

Helpers (run from the repository root):
- `.agents/skills/forgejo-audit/scripts/forgejo.sh`: the Forgejo client (shared with `forgejo-audit`)
- `.agents/skills/forgejo-verify/scripts/verify_scope.sh <n>`: what changed for one issue

## 1. Preflight

```bash
git fetch --quiet origin
.agents/skills/forgejo-audit/scripts/forgejo.sh check
.agents/skills/forgejo-audit/scripts/forgejo.sh ensure-labels
```

If the token is missing, follow the token instructions in `forgejo-audit` step 1.
Verify against committed HEAD. If `git status` shows uncommitted changes, tell the user they are
ignored. If HEAD is behind `origin`, say so, because a fix may already be pushed but not pulled.

## 2. Collect issues

```bash
.agents/skills/forgejo-audit/scripts/forgejo.sh list open audit        # default
.agents/skills/forgejo-audit/scripts/forgejo.sh list open -            # `all`
.agents/skills/forgejo-audit/scripts/forgejo.sh list all status/solved # always (default and `all`)
.agents/skills/forgejo-audit/scripts/forgejo.sh list closed status/verified-fixed   # `recheck`
```

`status/solved` is the label a developer (or the worker process) adds to say "I fixed this, please
verify". It is a claim, not a verdict, even though it shares the `status/` prefix with the verdict
labels. A `Closes #n` commit often closes the issue at the same time, so collect these issues from
both open and closed, and merge them with the rest of the list without duplicates.

All `status/` labels are exclusive, so an issue has only one at a time. Adding `status/solved`
replaces the previous verdict label. The previous verdict is still recorded in the newest
`audit-verify` marker in the issue's comments; read it from there when you need it, for example for
the report's "previous status" column.

## 3. Verify each issue

For each issue:

```bash
.agents/skills/forgejo-audit/scripts/forgejo.sh show <n>      # full body + comments
.agents/skills/forgejo-verify/scripts/verify_scope.sh <n>      # commits + diffs since reference
```

If `verify_scope.sh` prints **unchanged** (nothing was committed since the last verification) and the
issue already has a verdict label (`status/verified-fixed`, `status/partially-fixed` or
`status/not-fixed`), keep the previous verdict without a new comment. This shortcut never applies to
an issue that has `status/solved`: always verify a claim.

Otherwise, judge the issue against the code **at HEAD**, not only the diff:
1. Re-read the **Problem** and the **How to verify the fix** sections. The latter is the acceptance
   criterion. For hand-filed issues without one, use the reported behaviour. Then read the
   requirements the issue cites in `requirements.md` (the source of truth). If there are none, find
   the ones that cover the behaviour.
   - If `requirements.md` now contradicts the issue (the scope changed after it was filed), the
     requirements win. Judge against them. If the issue has become moot (out of scope or an accepted
     limitation), give no verdict: list it under "needs a human" with a suggestion to close it as not
     planned, citing the requirement.
2. Open every cited location at HEAD. Code moves, so if lines shifted, find the same logic by
   searching (`grep -rn`) instead of trusting line numbers.
3. Look for the fix anywhere, not just the cited files. A fix can live in a shared helper, middleware,
   a migration, config or nginx. Commits that mention `#n` are strong leads, and so are the
   `commit_refs` in `show` output. Forgejo shows these commits in the issue's comment stream, but they
   are timeline events, not comments.
4. Check that the fix is complete: every location listed, every path through the code (for example
   both the create and the update handler), and no new bypass. The fix must not break any other
   requirement in `requirements.md`; if it does, that's a new defect to file (see "Record the verdict"). If a test was added
   for it, run it if it's cheap, and mention whether you ran it.
   **Never run the backend tests against the default database:** without `DATABASE_URL` they use the
   live `gnucash` DB as the real user, and some tests wipe or change data. Use a throwaway database:
   ```bash
   podman exec gnucash-postgres psql -U gnucash -d postgres -c "CREATE DATABASE gnucash_verify_tmp"
   cd backend && DATABASE_URL=postgres://gnucash:gnucash_secret@127.0.0.1:5432/gnucash_verify_tmp \
     cargo test <name> -- --test-threads=1
   podman exec gnucash-postgres psql -U gnucash -d postgres -c "DROP DATABASE gnucash_verify_tmp"
   ```
   A fresh database has no seeded chart of accounts, so `test_get_accounts_and_tree`,
   `test_cannot_post_to_placeholder_account` and `test_account_register_running_balance` fail there.
   That's expected.

Verdict:
- **verified-fixed**: the defect can no longer occur at HEAD and the acceptance criterion is met.
- **partially-fixed**: some locations or paths are fixed, others aren't, or the fix has a gap.
- **not-fixed**: the defect is still there unchanged, or the attempted fix doesn't work.

Never mark an issue fixed just because a commit message or a `status/solved` label claims it. The
code must show it. If you
can't decide (the issue is too vague, or the code it describes is gone for unrelated reasons), leave
the status unchanged and list it under "needs a human" in the report.

## 4. Record the verdict

Skip this step for `dry-run`. Write the comment to a temp file in the scratchpad:

```markdown
**Verification at `<short-sha>`: ✅ Fixed** <!-- or ⚠️ Partially fixed / ❌ Not fixed -->

**Evidence**
- Requirement: `SEC-7` (current password required to change it).
- `backend/src/handlers/auth.rs:498` — `current_password` is now a required `String` and is always checked with `verify_password`.
- Fixed in <short-sha> "<commit subject>".

**Remaining** <!-- only for partially / not fixed -->
- `frontend/src/components/ChangePasswordModal.tsx:48` still sends the field as optional.

<!-- audit-verify: commit=<full-40-char-HEAD-sha> verdict=verified-fixed -->
```

Keep the `audit-verify` marker exactly as shown. `verify_scope.sh` uses it as the next reference
commit. Then post it and set the status:

```bash
.agents/skills/forgejo-audit/scripts/forgejo.sh comment <n> /path/to/comment.md
.agents/skills/forgejo-audit/scripts/forgejo.sh set-status <n> verified-fixed   # closes the issue
.agents/skills/forgejo-audit/scripts/forgejo.sh set-status <n> partially-fixed  # keeps/reopens
.agents/skills/forgejo-audit/scripts/forgejo.sh set-status <n> not-fixed        # keeps/reopens
```

To avoid noise, skip the comment when the verdict is **not-fixed**, the previous verdict was also
`not-fixed`, and none of the issue's code changed since the reference. Still post the comment if this
is the first verification, which has no verdict label yet.

`set-status` reopens a closed issue when the verdict isn't `verified-fixed`, which is how `recheck`
flags regressions. Call that out in the report.

`set-status` also removes `status/solved` whatever the verdict, so a
developer's claim is replaced by the verified status: `status/verified-fixed` and closed, or
`partially-fixed` / `not-fixed` and reopened. If a verdict rejects a `status/solved` claim, always
post the comment (even when the
not-fixed noise rule above would skip it) so the developer can see what is still missing.

If verifying a fix turns up a different defect (for example, the fix overcorrects or opens a new
hole), don't keep the original issue open for it. File a new issue with the `forgejo-audit` template
and `create` command, using the HEAD sha as the audit commit and `verify #<n>` in place of the audit
tag. Then link it from the verification comment.

## 5. Report

Give the user a table with the issue number, title, the previous status (including `status/solved`)
and the new verdict, and a one-line reason. Then add totals (fixed / partial / not fixed / needs a human), the HEAD commit
verified, and links to the issues that changed state.
