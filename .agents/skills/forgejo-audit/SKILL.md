---
name: forgejo-audit
description: >-
  Audit GnuCash Web code for security, correctness, data-integrity and performance defects
  and file each verified finding as an issue in the Forgejo bug tracker. Incremental: audits only
  the diff since the previous audit (tracked by `audit/*` git tags), or the full tree on the first
  run. Use when the user asks to audit the code, review changes since the last audit, or file audit
  issues in Forgejo.
argument-hint: "[full] [dry-run]"
---

# Forgejo Code Audit

Audit the code, file one Forgejo issue per verified defect, then tag the audited commit so the next
run only looks at what changed since this one.

Arguments (`$ARGUMENTS`):
- `full` audits the whole tree even when a previous `audit/*` tag exists.
- `dry-run` reports findings in the terminal and creates no issues and no tag.

Helper: `.agents/skills/forgejo-audit/scripts/forgejo.sh` (run with no arguments for usage).
Run every command from the repository root.

## 1. Preflight

```bash
.agents/skills/forgejo-audit/scripts/forgejo.sh check
.agents/skills/forgejo-audit/scripts/forgejo.sh ensure-labels
```

If `check` fails with "no API token", stop and tell the user to create a token in Forgejo
(*Settings → Applications → Generate new token*, scopes **issue: read & write** and
**repository: read**) and save it:

```bash
mkdir -p ~/.config/forgejo && install -m 600 /dev/stdin ~/.config/forgejo/token <<<'<token>'
```

Skip this step for `dry-run`, except still run `check` to read existing issues if a token exists.

## 2. Determine scope

```bash
.agents/skills/forgejo-audit/scripts/audit_scope.sh          # add --full for `full`
```

- **base: none** means this is a full audit: review every listed file.
- **base: audit/...** means this is an incremental audit: read `git diff <base> HEAD` for the listed
  files, plus enough of the surrounding code to judge each change (callers, the handler's auth
  checks, the SQL it builds, the component's API call). Defects in untouched code are out of scope,
  unless a change makes them newly reachable.
- **nothing to audit** means stop and report that HEAD has already been audited.

Then load the existing tracker state so you don't re-file known problems:

```bash
.agents/skills/forgejo-audit/scripts/forgejo.sh list all
```

## 3. Audit

Read `requirements.md` first. It's the source of truth for expected behaviour, and its requirement IDs
(e.g. `ACC-2`) are what findings cite. Then read `AGENTS.md` for the architecture. Check, in this order:

0. **Requirements conformance**: any behaviour that breaks a `must` in `requirements.md` is a defect,
   even if it looks deliberate in the code. Name the requirement ID in the issue.

1. **Security**: authn/authz on every Axum handler (`backend/src/handlers/`), tenant isolation
   (every query filtered by `user_id`), SQL built from strings, JWT/admin-key handling, secrets in
   `docker-compose.yml`, `.env*` and the scripts, upload/import parsing, CORS, nginx headers, XSS in
   React (`dangerouslySetInnerHTML`, unescaped URLs), and tokens in `localStorage`.
2. **Data integrity**: double-entry balance (splits sum to zero), money as integer or rational and
   never float, transactions around multi-statement writes, import/export round-trips (GnuCash
   SQLite/XML), migrations.
3. **Correctness**: error handling (`unwrap`/`expect` on request paths), off-by-one date ranges in
   reports, stale React state, races.
4. **Performance**: N+1 queries, missing indexes for the filters used, unbounded result sets, and
   large re-renders.

Rules for a finding:
- **Respect the requirements' scope.** Don't file behaviour that `requirements.md` marks as out of
  scope or as an accepted limitation (e.g. `INV-6`), or that falls under its **Open questions**.
  Don't invent a requirement either: if you find behaviour the file doesn't settle, list it in the
  report as a suggested new open question instead of filing it.
- **Verify before filing.** Open the code and confirm the defect is real at HEAD. Trace the call path.
  Drop anything speculative. A short list of real issues beats a long list of maybes.
- One issue per root cause. Merge several symptoms of the same mistake into one issue.
- Cite exact `path:line` locations at HEAD.
- Pick a stable **fingerprint**: `<primary-file>#<kebab-slug-of-the-defect>`, for example
  `backend/src/handlers/auth.rs#change-password-skips-current-password`. The fingerprint must stay the
  same if the same defect is found again, so describe the defect, not the line number.

Dedupe against the `list all` output:
- **Open issue with the same fingerprint or the same root cause**: don't file it again. If the diff
  made it worse or added a new location, add a comment to that issue instead (`forgejo.sh comment`).
- **Closed `status/verified-fixed` issue that has come back**: file a new issue titled
  `Regression: …` that links the old one (`Regression of #N`).
- **Closed without `status/verified-fixed`** (closed by hand as won't-fix or invalid): don't refile it.

## 4. File issues

For each finding, write the body to a temp file (in the scratchpad), using this template exactly.
The two HTML comments are machine-read by `forgejo.sh` and the `forgejo-verify` skill, so keep them.

```markdown
**Severity:** High · **Kind:** security · **Requirement:** `SEC-7` · **Found in:** `<short-sha>` (audit `audit/<timestamp>`)

### Problem
<What is wrong and why it matters. One short paragraph.>

### Location
- `backend/src/handlers/auth.rs:500-512`
- `frontend/src/components/ChangePasswordModal.tsx:48`

### Impact / failure scenario
<Concrete inputs → wrong outcome. Who can trigger it.>

### Suggested fix
<Specific change. Small code snippet if it helps.>

### How to verify the fix
<What the code must look like, or the test that must pass, for this to count as solved.>

<!-- audit-fingerprint: backend/src/handlers/auth.rs#change-password-skips-current-password -->
<!-- audit-commit: <full-40-char-HEAD-sha> -->
```

The **How to verify the fix** section is what `forgejo-verify` checks against later, so make it testable
and consistent with the cited requirement. Put every requirement the defect breaks in the
**Requirement** field (comma-separated). Use `none` only for defects no requirement covers (for
example, a performance problem). In that case, add a suggested requirement to the report.

Create the issue. The title is `[<Severity>] <imperative summary>`, at most 80 characters:

```bash
.agents/skills/forgejo-audit/scripts/forgejo.sh create \
  "[High] Require current password in change-password" /path/to/body.md \
  audit severity/high kind/security
```

Labels: always `audit`, exactly one `severity/*`, and one or more `kind/*`. Exit code 3 means a
duplicate open issue exists: switch to commenting on it.

## 5. Record the audit

After all issues are filed (not for `dry-run`), tag HEAD so the next audit starts from here, then push
the tag so other machines share the baseline:

```bash
tag="audit/$(date -u +%Y%m%dT%H%M%SZ)"
git tag -a "$tag" -m "forgejo-audit: <N> new issues (<#a, #b, ...>), base <previous tag or 'none'>" HEAD
git push origin "$tag"
```

Tag even when there are zero findings: a clean audit is still an audit.

## 6. Report

Give the user a short summary with the scope (base → HEAD, number of files), a table of the new
issues (number, severity, requirement, title, link), any existing issues you commented on, and the tag
that was created. Then list any suggested additions or open questions for `requirements.md`. Don't
edit that file yourself: the user owns it. For `dry-run`, show the findings table and the issue titles that would have been filed.
