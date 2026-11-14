#!/usr/bin/env bash
# Print the scope of the next audit: the previous audit tag (if any), HEAD, and the
# changed source files between them. With no previous audit tag the scope is the full tree.
#
# Usage: audit_scope.sh [--full]     --full ignores the previous audit tag
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

# Ignore generated, binary and vendored paths; they are never audited.
EXCLUDE=(':!**/screenshots/**' ':!*.png' ':!*.jpg' ':!*.gnucash' ':!*.sqlite' ':!**/package-lock.json'
  ':!**/Cargo.lock' ':!**/node_modules/**' ':!**/target/**' ':!frontend/dist/**')

git fetch --quiet --tags origin 2>/dev/null || echo "warning: could not fetch tags from origin" >&2

head=$(git rev-parse HEAD)
base_tag=""
[[ ${1:-} == --full ]] || base_tag=$(git tag --list 'audit/*' --sort=-creatordate | head -1)

echo "head:     $head ($(git log -1 --format='%h %s' HEAD))"
if [[ -n $(git status --porcelain --untracked-files=no) ]]; then
  echo "warning:  working tree has uncommitted changes; the audit covers committed HEAD only"
fi

if [[ -z $base_tag ]]; then
  echo "base:     none (full audit)"
  echo "files:"
  git ls-files -- . "${EXCLUDE[@]}" | sed 's/^/  /'
  exit 0
fi

base=$(git rev-list -n1 "$base_tag")
echo "base:     $base_tag -> $base ($(git log -1 --format='%h %s' "$base"))"
if [[ $base == "$head" ]]; then
  echo "nothing to audit: HEAD is the previously audited commit"
  exit 0
fi
echo "commits:"
git log --format='  %h %s' "$base..$head"
echo "files:"
git diff --stat=200 "$base" "$head" -- . "${EXCLUDE[@]}" | sed 's/^/  /'
