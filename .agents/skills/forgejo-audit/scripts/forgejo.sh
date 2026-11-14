#!/usr/bin/env bash
# Thin Forgejo issue-tracker client shared by the forgejo-audit and forgejo-verify skills.
# Requires: curl, jq, git.
#
# Config (env):
#   FORGEJO_URL         default http://192.168.2.2:3001
#   FORGEJO_REPO        default owner/repo parsed from `git remote get-url origin`
#   FORGEJO_TOKEN       API token (scopes: write:issue, read:repository)
#   FORGEJO_TOKEN_FILE  default ~/.config/forgejo/token (used when FORGEJO_TOKEN is unset)
#
# Usage: forgejo.sh <command> [args]
#   check                                   verify URL, repo and token
#   ensure-labels                           create the audit/severity/kind/status labels if missing
#   list [open|closed|all] [label|-]        audit issues (or any label, '-' = all issues) as compact JSON
#   show <n>                                issue + comments + commits referencing it (timeline) as JSON
#   create <title> <body-file> <label>...   create issue; refuses if an open issue has the same fingerprint
#   comment <n> <body-file>                 add a comment
#   set-status <n> <verified-fixed|partially-fixed|not-fixed>
#                                           swap status/* label (and drop status:solved);
#                                           close on verified-fixed, reopen otherwise
set -euo pipefail

die() { echo "forgejo.sh: $*" >&2; exit 1; }

: "${FORGEJO_URL:=http://192.168.2.2:3001}"
FORGEJO_URL=${FORGEJO_URL%/}
if [[ -z "${FORGEJO_REPO:-}" ]]; then
  remote=$(git remote get-url origin 2>/dev/null || true)
  FORGEJO_REPO=$(sed -E 's#\.git$##; s#^.*[:/]([^/:]+/[^/]+)$#\1#' <<<"$remote")
  [[ $FORGEJO_REPO == */* ]] || die "cannot derive owner/repo from origin '$remote'; set FORGEJO_REPO"
fi
REPO_API="/repos/$FORGEJO_REPO"

load_token() {
  [[ -n "${FORGEJO_TOKEN:-}" ]] && return
  local f="${FORGEJO_TOKEN_FILE:-$HOME/.config/forgejo/token}"
  [[ -r $f ]] || die "no API token: export FORGEJO_TOKEN or write one to $f (chmod 600)"
  FORGEJO_TOKEN=$(tr -d '[:space:]' <"$f")
  [[ -n $FORGEJO_TOKEN ]] || die "token file $f is empty"
}

# api METHOD PATH [JSON]  -> response body on stdout, dies on HTTP >= 400
api() {
  local method=$1 path=$2 data=${3:-} out code
  local args=(-sS -X "$method" -H "Authorization: token $FORGEJO_TOKEN" -H 'Accept: application/json' -w $'\n%{http_code}')
  [[ -n $data ]] && args+=(-H 'Content-Type: application/json' --data-binary "$data")
  out=$(curl "${args[@]}" "$FORGEJO_URL/api/v1$path") || die "curl failed for $method $path"
  code=${out##*$'\n'}
  out=${out%$'\n'*}
  (( code < 400 )) || die "$method $path -> HTTP $code: $out"
  printf '%s\n' "$out"
}

# list_all PATH -> every page concatenated into one JSON array
list_all() {
  local path=$1 sep='?' page=1 acc='[]' chunk
  [[ $path == *\?* ]] && sep='&'
  while :; do
    chunk=$(api GET "$path${sep}limit=50&page=$page")
    acc=$(jq -c --argjson c "$chunk" '. + $c' <<<"$acc")
    (( $(jq length <<<"$chunk") < 50 )) && break
    page=$((page + 1))
  done
  printf '%s\n' "$acc"
}

# label_ids NAME... -> JSON array of ids; dies if any name is unknown
label_ids() {
  local all names
  all=$(list_all "$REPO_API/labels")
  names=$(printf '%s\n' "$@" | jq -R . | jq -sc .)
  jq -c --argjson want "$names" '
    ($want - [.[].name]) as $missing
    | if ($missing | length) > 0 then error("unknown labels: \($missing | join(", ")) (run ensure-labels)")
      else [.[] | select(.name as $n | $want | index($n)) | .id] end' <<<"$all"
}

# name|color|exclusive|description
LABELS=(
  "audit|#5319e7|false|Found by the forgejo-audit skill"
  "severity/critical|#b60205|true|Exploitable or data-destroying; fix immediately"
  "severity/high|#d93f0b|true|Serious bug or security weakness"
  "severity/medium|#fbca04|true|Real defect with limited impact"
  "severity/low|#c5def5|true|Hardening, minor bug or cleanup"
  "kind/security|#e11d21|false|Security vulnerability or hardening"
  "kind/bug|#ee0701|false|Incorrect behaviour"
  "kind/data-integrity|#a2238d|false|Ledger, import/export or persistence correctness"
  "kind/performance|#1d76db|false|Speed, memory or query efficiency"
  "kind/maintainability|#bfd4f2|false|Dead code, duplication, fragile design"
  "status/not-fixed|#d93f0b|true|Last verification found the issue still present"
  "status/partially-fixed|#fbca04|true|Last verification found an incomplete fix"
  "status/verified-fixed|#0e8a16|true|Last verification confirmed the fix"
  "status:solved|#c2e0c6|false|Developer says this is fixed; awaiting forgejo-verify"
)

cmd_check() {
  echo "url:  $FORGEJO_URL"
  echo "repo: $FORGEJO_REPO"
  local login repo
  login=$(api GET /user | jq -r .login)
  repo=$(api GET "$REPO_API")
  echo "user: $login"
  jq -r '"issues enabled: \(.has_issues)\npermissions: \(.permissions)"' <<<"$repo"
}

cmd_ensure_labels() {
  local existing entry name color excl desc
  existing=$(list_all "$REPO_API/labels" | jq -r '.[].name')
  for entry in "${LABELS[@]}"; do
    IFS='|' read -r name color excl desc <<<"$entry"
    if grep -qxF "$name" <<<"$existing"; then continue; fi
    api POST "$REPO_API/labels" "$(jq -nc --arg n "$name" --arg c "$color" --arg d "$desc" --argjson e "$excl" \
      '{name: $n, color: $c, description: $d, exclusive: $e}')" >/dev/null
    echo "created label $name"
  done
  echo "labels ok"
}

cmd_list() {
  local state=${1:-open} label=${2:-audit} q
  q="$REPO_API/issues?type=issues&state=$state"
  [[ $label != - ]] && q+="&labels=$(jq -rn --arg l "$label" '$l | @uri')"
  list_all "$q" | jq -c '[.[] | {
      number, title, state,
      labels: [.labels[].name],
      created_at, closed_at,
      fingerprint: ([.body | capture("audit-fingerprint: (?<f>\\S+) -->")? | .f][0] // null),
      audit_commit: ([.body | capture("audit-commit: (?<c>[0-9a-f]+) -->")? | .c][0] // null)
    }] | sort_by(.number) | .[]'
}

cmd_show() {
  local n=${1:?issue number}
  jq -n --argjson i "$(api GET "$REPO_API/issues/$n")" --argjson c "$(list_all "$REPO_API/issues/$n/comments")" \
    --argjson t "$(list_all "$REPO_API/issues/$n/timeline")" '{
    number: $i.number, title: $i.title, state: $i.state, labels: [$i.labels[].name],
    created_at: $i.created_at, body: $i.body,
    comments: [$c[] | {id, user: .user.login, created_at, body}],
    commit_refs: [$t[] | select(.type == "commit_ref") | {sha: .ref_commit_sha, user: .user.login, created_at}]
  }'
}

cmd_create() {
  local title=${1:?title} body_file=${2:?body file}; shift 2
  [[ -r $body_file ]] || die "cannot read $body_file"
  local body fp dup ids
  body=$(<"$body_file")
  fp=$(grep -oP 'audit-fingerprint: \K\S+(?= -->)' <<<"$body" | head -1 || true)
  if [[ -n $fp ]]; then
    dup=$(cmd_list open - | jq -r --arg f "$fp" 'select(.fingerprint == $f) | .number' | head -1)
    [[ -z $dup ]] || { echo "duplicate: open issue #$dup already has fingerprint $fp" >&2; exit 3; }
  fi
  ids=$(label_ids "$@")
  api POST "$REPO_API/issues" "$(jq -nc --arg t "$title" --arg b "$body" --argjson l "$ids" \
    '{title: $t, body: $b, labels: $l}')" | jq -r '"#\(.number) \(.html_url)"'
}

cmd_comment() {
  local n=${1:?issue number} body_file=${2:?body file}
  [[ -r $body_file ]] || die "cannot read $body_file"
  api POST "$REPO_API/issues/$n/comments" "$(jq -nc --rawfile b "$body_file" '{body: $b}')" | jq -r .html_url
}

cmd_set_status() {
  local n=${1:?issue number} status=${2:?status} want state
  case $status in verified-fixed) state=closed ;; partially-fixed | not-fixed) state=open ;;
    *) die "status must be verified-fixed, partially-fixed or not-fixed" ;; esac
  want=$(label_ids "status/$status" | jq '.[0]')
  # drop any other status/* label and the developer's status:solved claim, then add the new one
  api GET "$REPO_API/issues/$n/labels" | jq -r --argjson w "$want" \
    '.[] | select((.name | startswith("status/") or . == "status:solved") and .id != $w) | .id' |
    while read -r id; do api DELETE "$REPO_API/issues/$n/labels/$id" >/dev/null; done
  api POST "$REPO_API/issues/$n/labels" "{\"labels\":[$want]}" >/dev/null
  api PATCH "$REPO_API/issues/$n" "{\"state\":\"$state\"}" | jq -r '"#\(.number) \(.state) status/'"$status"'"'
}

cmd=${1:-}; shift || true
case $cmd in check | ensure-labels | list | show | create | comment | set-status) load_token ;; esac
case $cmd in
  check) cmd_check ;;
  ensure-labels) cmd_ensure_labels ;;
  list) cmd_list "$@" ;;
  show) cmd_show "$@" ;;
  create) cmd_create "$@" ;;
  comment) cmd_comment "$@" ;;
  set-status) cmd_set_status "$@" ;;
  *) sed -n '2,/^set -e/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 1 ;;
esac
