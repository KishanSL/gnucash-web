#!/usr/bin/env bash
# For one Forgejo issue, print what changed in the code since it was last looked at:
# the reference commit, commits that mention the issue, and diffs of the files the issue cites.
#
# Reference commit, first match wins:
#   1. newest `<!-- audit-verify: commit=SHA ... -->` marker in the issue's comments
#   2. `<!-- audit-commit: SHA -->` in the issue body
#   3. last commit before the issue was created
#
# Usage: verify_scope.sh <issue-number>
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
n=${1:?issue number}
FJ=.agents/skills/forgejo-audit/scripts/forgejo.sh

issue=$($FJ show "$n")
head=$(git rev-parse HEAD)

ref=$(jq -r '[.comments[].body | capture("audit-verify: commit=(?<c>[0-9a-f]+)")? | .c] | last // empty' <<<"$issue")
src="last verification"
if [[ -z $ref ]]; then
  ref=$(jq -r '[.body | capture("audit-commit: (?<c>[0-9a-f]+) -->")? | .c][0] // empty' <<<"$issue")
  src="audit commit"
fi
if [[ -z $ref ]]; then
  ref=$(git rev-list -n1 --before="$(jq -r .created_at <<<"$issue")" HEAD)
  src="issue creation date"
fi
git cat-file -e "$ref^{commit}" 2>/dev/null || { echo "reference commit $ref not found locally; run git fetch" >&2; exit 1; }

echo "issue:     #$n $(jq -r .title <<<"$issue") [$(jq -r '.state + " " + (.labels | join(","))' <<<"$issue")]"
echo "reference: $ref ($src)"
echo "head:      $head"
if [[ $(git rev-parse "$ref") == "$head" ]]; then
  echo "unchanged: HEAD is the reference commit"
  exit 0
fi

echo
echo "commits since reference:"
git log --format='  %h %s' "$ref..$head" | head -50

echo
echo "commits mentioning #$n:"
git log --format='  %h %s' -E --grep="(^|[^0-9&])#$n([^0-9]|$)" "$ref..$head" || true

echo
echo "commits referenced on the issue timeline:"
jq -r '.commit_refs[].sha' <<<"$issue" | while read -r sha; do
  if git cat-file -e "$sha^{commit}" 2>/dev/null; then git log -1 --format='  %h %s' "$sha"; else echo "  $sha (not found locally; run git fetch)"; fi
done

# Paths cited in the issue body as `path` or `path:line`.
mapfile -t cited < <(jq -r .body <<<"$issue" | grep -oP '`\K[A-Za-z0-9_./-]+\.[A-Za-z0-9]+(?=(:[0-9-]+)?`)' | sort -u)
existing=()
for p in "${cited[@]}"; do [[ -e $p ]] && existing+=("$p") || echo "cited path no longer exists: $p"; done

echo
if (( ${#existing[@]} )); then
  echo "diff of cited files (${existing[*]}):"
  git diff "$ref" "$head" -- "${existing[@]}"
else
  echo "no cited files found in the issue body; inspect the code it describes directly"
fi
