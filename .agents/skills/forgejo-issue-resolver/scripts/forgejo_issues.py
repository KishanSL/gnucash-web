#!/usr/bin/env python3
"""
Forgejo / Gitea Issue Management Utility

Interacts with the Forgejo v1 REST API to list, inspect, comment on, and tag issues.
Supports filtering by issue kind (bug, performance, data-integrity, security)
and severity (low, medium, high).
Tags resolved issues with 'status/solved' without closing them.
Uses only the Python standard library (no external dependencies required).
"""

import argparse
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Dict, List, Optional, Tuple

VALID_KINDS = ["bug", "performance", "data-integrity", "security", "all"]
VALID_SEVERITIES = ["low", "medium", "high", "all"]


def get_git_remote_info() -> Tuple[Optional[str], Optional[str], Optional[str]]:
    """Auto-detects Forgejo host, owner, and repo name from git remote origin."""
    try:
        out = subprocess.check_output(
            ["git", "remote", "get-url", "origin"],
            stderr=subprocess.DEVNULL,
            text=True,
        ).strip()
    except Exception:
        return None, None, None

    # Matches ssh://git@192.168.2.2:2222/owner/repo.git or git@host:owner/repo.git
    ssh_match = re.search(r"@([^:/]+)(?::\d+)?[:/]([^/]+)/([^/]+?)(?:\.git)?$", out)
    if ssh_match:
        host = ssh_match.group(1)
        owner = ssh_match.group(2)
        repo = ssh_match.group(3)
        return host, owner, repo

    # Matches http://host:port/owner/repo.git
    http_match = re.search(r"https?://([^:/]+)(?::\d+)?/([^/]+)/([^/]+?)(?:\.git)?$", out)
    if http_match:
        host = http_match.group(1)
        owner = http_match.group(2)
        repo = http_match.group(3)
        return host, owner, repo

    return None, None, None


def resolve_forgejo_url(custom_url: Optional[str], detected_host: Optional[str]) -> str:
    """Resolves the Forgejo base URL."""
    if custom_url:
        return custom_url.rstrip("/")

    env_url = os.environ.get("FORGEJO_URL") or os.environ.get("GITEA_URL")
    if env_url:
        return env_url.rstrip("/")

    if detected_host:
        # Default LAN host port mapping (e.g. 192.168.2.2 defaults to :3001)
        if detected_host == "192.168.2.2":
            return "http://192.168.2.2:3001"
        return f"http://{detected_host}:3000"

    return "http://192.168.2.2:3001"


def get_auth_token(custom_token: Optional[str]) -> Optional[str]:
    """Retrieves the Forgejo Personal Access Token."""
    if custom_token:
        return custom_token

    env_token = os.environ.get("FORGEJO_TOKEN") or os.environ.get("GITEA_TOKEN")
    if env_token:
        return env_token

    # Check ~/.config/forgejo/token
    token_file = os.path.expanduser("~/.config/forgejo/token")
    if os.path.isfile(token_file):
        try:
            with open(token_file, "r") as f:
                token = f.read().strip()
                if token:
                    return token
        except Exception:
            pass

    return None


def make_api_request(
    url: str,
    method: str = "GET",
    token: Optional[str] = None,
    data: Optional[Dict[str, Any]] = None,
) -> Any:
    """Executes an HTTP request to the Forgejo API."""
    headers = {
        "Accept": "application/json",
        "User-Agent": "antigravity-forgejo-client/1.0",
    }
    if token:
        headers["Authorization"] = f"token {token}"

    body_bytes = None
    if data is not None:
        headers["Content-Type"] = "application/json"
        body_bytes = json.dumps(data).encode("utf-8")

    req = urllib.request.Request(url, data=body_bytes, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            content = resp.read().decode("utf-8")
            if not content:
                return {}
            return json.loads(content)
    except urllib.error.HTTPError as e:
        error_msg = e.read().decode("utf-8", errors="ignore")
        print(f"API Error ({e.code}): {error_msg}", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Network error: {e}", file=sys.stderr)
        sys.exit(1)


def ensure_label(
    base_url: str,
    token: str,
    owner: str,
    repo: str,
    label_name: str,
    color: str = "0e8a16",
    description: str = "",
) -> int:
    """Finds or creates a label by name and returns its ID."""
    labels_url = f"{base_url}/api/v1/repos/{owner}/{repo}/labels"
    existing_labels = make_api_request(labels_url, method="GET", token=token)
    if isinstance(existing_labels, list):
        for lbl in existing_labels:
            if lbl.get("name") == label_name:
                return lbl.get("id")

    # Create label if not found
    create_data = {
        "name": label_name,
        "color": color,
        "description": description,
    }
    created = make_api_request(labels_url, method="POST", token=token, data=create_data)
    return created.get("id")


def classify_issue(issue: Dict[str, Any]) -> Tuple[str, str]:
    """
    Classifies an issue into:
    - kind: 'security', 'data-integrity', 'performance', 'bug'
    - severity: 'high', 'medium', 'low'
    """
    labels = [lbl.get("name", "").lower() for lbl in issue.get("labels", [])]
    title = (issue.get("title") or "").lower()
    body = (issue.get("body") or "").lower()
    text = f"{title} {body}"

    # 1. Determine Kind
    kind = "bug"  # default fallback

    # Explicit labels check
    if any(l in ("security", "kind/security", "sec", "vulnerability") for l in labels):
        kind = "security"
    elif any(l in ("data-integrity", "kind/data-integrity", "integrity", "financial-integrity") for l in labels):
        kind = "data-integrity"
    elif any(l in ("performance", "kind/performance", "perf", "kind/perf", "optimization") for l in labels):
        kind = "performance"
    elif any(l in ("bug", "kind/bug", "defect", "error") for l in labels):
        kind = "bug"
    # Text-based heuristic check
    elif re.search(r"\b(security|vulnerability|cve|auth|bypass|jwt|injection|xss|csrf|constant-time|privilege)\b", text):
        kind = "security"
    elif re.search(r"\b(data-integrity|integrity|balance|imbalance|precision|rounding|decimal|double-entry|cycle|reconciliation)\b", text):
        kind = "data-integrity"
    elif re.search(r"\b(performance|latency|slow|throughput|optimize|caching|bottleneck|n\+1|memory leak|speed)\b", text):
        kind = "performance"
    elif re.search(r"\b(bug|error|panic|crash|failure|broken|defect|fault)\b", text):
        kind = "bug"

    # 2. Determine Severity
    severity = "medium"  # default fallback

    # Explicit labels check
    if any(l in ("critical", "severity/critical", "high", "severity/high", "urgent", "p0", "p1", "blocker") for l in labels):
        severity = "high"
    elif any(l in ("medium", "severity/medium", "moderate", "p2") for l in labels):
        severity = "medium"
    elif any(l in ("low", "severity/low", "minor", "p3", "p4", "trivial", "hardening") for l in labels):
        severity = "low"
    # Text-based heuristic check
    elif re.search(r"\b(critical|high|fatal|urgent|severe|blocker|p0|p1|c1|c2|c3|h1|h2|h3|rce|takeover)\b", text):
        severity = "high"
    elif re.search(r"\b(low|minor|trivial|p3|p4|l1|l2|l3|l4|l5|cosmetic|hardening)\b", text):
        severity = "low"
    elif re.search(r"\b(medium|moderate|p2|m1|m2|m3|m7|m8|m9|m10)\b", text):
        severity = "medium"

    return kind, severity


def cmd_list(args: argparse.Namespace) -> None:
    """Lists open issues with kind, severity filtering and limits."""
    base_url = resolve_forgejo_url(args.url, args.detected_host)
    token = get_auth_token(args.token)
    owner = args.owner or args.detected_owner
    repo = args.repo or args.detected_repo

    if not owner or not repo:
        print("Error: Could not determine owner and repo. Specify --owner and --repo.", file=sys.stderr)
        sys.exit(1)

    url = f"{base_url}/api/v1/repos/{owner}/{repo}/issues?state=open"
    issues = make_api_request(url, method="GET", token=token)

    if not isinstance(issues, list):
        print(f"Unexpected response: {issues}")
        return

    # Filter out pull requests
    issues = [i for i in issues if not i.get("pull_request")]

    # By default, exclude issues already tagged with status/solved unless --include-solved is set
    if not getattr(args, "include_solved", False):
        issues = [
            i
            for i in issues
            if not any(
                lbl.get("name") in ("status/solved", "status:solved", "status/verified-fixed")
                for lbl in i.get("labels", [])
            )
        ]

    target_kind = (args.kind or args.type or "all").lower()
    target_severity = (args.severity or "all").lower()

    filtered = []
    for issue in issues:
        detected_kind, detected_severity = classify_issue(issue)
        issue["detected_kind"] = detected_kind
        issue["detected_severity"] = detected_severity

        # Check kind match
        kind_matches = False
        if target_kind in ("all", "any"):
            kind_matches = True
        elif target_kind == detected_kind:
            kind_matches = True
        elif target_kind in ("feature", "enhancement") and detected_kind not in ("bug", "security", "data-integrity"):
            kind_matches = True

        # Check severity match
        severity_matches = False
        if target_severity in ("all", "any"):
            severity_matches = True
        elif target_severity == detected_severity:
            severity_matches = True

        if kind_matches and severity_matches:
            filtered.append(issue)

    if args.limit and args.limit > 0:
        filtered = filtered[: args.limit]

    output = {
        "repository": f"{owner}/{repo}",
        "server": base_url,
        "filter_kind": target_kind,
        "filter_severity": target_severity,
        "total_matched": len(filtered),
        "issues": [
            {
                "number": i.get("number"),
                "title": i.get("title"),
                "kind": i.get("detected_kind"),
                "severity": i.get("detected_severity"),
                "labels": [lbl.get("name") for lbl in i.get("labels", [])],
                "created_at": i.get("created_at"),
                "author": i.get("user", {}).get("login"),
                "body_preview": (i.get("body") or "")[:200].replace("\n", " "),
            }
            for i in filtered
        ],
    }

    print(json.dumps(output, indent=2))


def cmd_get(args: argparse.Namespace) -> None:
    """Gets full issue details, detected classification, and comments."""
    base_url = resolve_forgejo_url(args.url, args.detected_host)
    token = get_auth_token(args.token)
    owner = args.owner or args.detected_owner
    repo = args.repo or args.detected_repo

    if not owner or not repo:
        print("Error: Could not determine owner and repo. Specify --owner and --repo.", file=sys.stderr)
        sys.exit(1)

    issue_url = f"{base_url}/api/v1/repos/{owner}/{repo}/issues/{args.issue_number}"
    comments_url = f"{base_url}/api/v1/repos/{owner}/{repo}/issues/{args.issue_number}/comments"

    issue = make_api_request(issue_url, method="GET", token=token)
    comments = make_api_request(comments_url, method="GET", token=token)

    detected_kind, detected_severity = classify_issue(issue)

    output = {
        "number": issue.get("number"),
        "title": issue.get("title"),
        "state": issue.get("state"),
        "kind": detected_kind,
        "severity": detected_severity,
        "author": issue.get("user", {}).get("login"),
        "labels": [lbl.get("name") for lbl in issue.get("labels", [])],
        "created_at": issue.get("created_at"),
        "body": issue.get("body"),
        "comments": [
            {
                "author": c.get("user", {}).get("login"),
                "created_at": c.get("created_at"),
                "body": c.get("body"),
            }
            for c in (comments if isinstance(comments, list) else [])
        ],
    }
    print(json.dumps(output, indent=2))


def cmd_comment(args: argparse.Namespace) -> None:
    """Posts a comment on an issue."""
    base_url = resolve_forgejo_url(args.url, args.detected_host)
    token = get_auth_token(args.token)
    if not token:
        print("Error: FORGEJO_TOKEN is required to post comments.", file=sys.stderr)
        sys.exit(1)

    owner = args.owner or args.detected_owner
    repo = args.repo or args.detected_repo

    url = f"{base_url}/api/v1/repos/{owner}/{repo}/issues/{args.issue_number}/comments"
    data = {"body": args.body}
    resp = make_api_request(url, method="POST", token=token, data=data)
    print(json.dumps({"status": "success", "comment_id": resp.get("id")}, indent=2))


def cmd_solve(args: argparse.Namespace) -> None:
    """Marks an issue as solved by adding 'status/solved' tag and optional comment. DOES NOT close the issue."""
    base_url = resolve_forgejo_url(args.url, args.detected_host)
    token = get_auth_token(args.token)
    if not token:
        print("Error: FORGEJO_TOKEN is required to tag and update issues.", file=sys.stderr)
        sys.exit(1)

    owner = args.owner or args.detected_owner
    repo = args.repo or args.detected_repo

    target_label_name = "status/solved"

    # Ensure status/solved label exists in repo
    label_id = ensure_label(
        base_url,
        token,
        owner,
        repo,
        label_name=target_label_name,
        color="0e8a16",
        description="Issue has been solved and verified",
    )

    # Fetch current issue labels to identify and remove obsolete/conflicting status labels
    issue_url = f"{base_url}/api/v1/repos/{owner}/{repo}/issues/{args.issue_number}"
    current_issue = make_api_request(issue_url, method="GET", token=token)
    current_labels = current_issue.get("labels", []) if isinstance(current_issue, dict) else []

    for lbl in current_labels:
        lbl_name = (lbl.get("name") or "").lower()
        lbl_id = lbl.get("id")
        # Remove any other status labels (e.g. status/partially-fixed, status/not-fixed, status:solved, etc.)
        if (lbl_name.startswith("status/") or lbl_name.startswith("status:")) and lbl_name != target_label_name.lower():
            del_url = f"{base_url}/api/v1/repos/{owner}/{repo}/issues/{args.issue_number}/labels/{lbl_id}"
            try:
                make_api_request(del_url, method="DELETE", token=token)
            except Exception as e:
                print(f"Warning: Failed to remove old status label '{lbl.get('name')}': {e}", file=sys.stderr)

    # Attach status/solved label to issue (POST /issues/:id/labels)
    issue_labels_url = f"{base_url}/api/v1/repos/{owner}/{repo}/issues/{args.issue_number}/labels"
    make_api_request(issue_labels_url, method="POST", token=token, data={"labels": [label_id]})

    # Optionally post resolution comment
    if args.comment:
        comment_url = f"{base_url}/api/v1/repos/{owner}/{repo}/issues/{args.issue_number}/comments"
        make_api_request(comment_url, method="POST", token=token, data={"body": args.comment})

    # Fetch updated issue state to confirm it remains open and has the label
    issue = make_api_request(issue_url, method="GET", token=token)

    labels = [l.get("name") for l in issue.get("labels", [])]
    output = {
        "status": "solved",
        "issue_number": issue.get("number"),
        "state": issue.get("state"),
        "labels": labels,
        "message": f"Issue tagged with {target_label_name} (issue remains open as requested).",
    }
    print(json.dumps(output, indent=2))


def cmd_close(args: argparse.Namespace) -> None:
    """Closes an issue (legacy / explicit close command)."""
    base_url = resolve_forgejo_url(args.url, args.detected_host)
    token = get_auth_token(args.token)
    if not token:
        print("Error: FORGEJO_TOKEN is required to close issues.", file=sys.stderr)
        sys.exit(1)

    owner = args.owner or args.detected_owner
    repo = args.repo or args.detected_repo

    if args.comment:
        comment_url = f"{base_url}/api/v1/repos/{owner}/{repo}/issues/{args.issue_number}/comments"
        make_api_request(comment_url, method="POST", token=token, data={"body": args.comment})

    issue_url = f"{base_url}/api/v1/repos/{owner}/{repo}/issues/{args.issue_number}"
    data = {"state": "closed"}
    resp = make_api_request(issue_url, method="PATCH", token=token, data=data)
    print(json.dumps({"status": "closed", "issue_number": resp.get("number"), "state": resp.get("state")}, indent=2))


def main() -> None:
    detected_host, detected_owner, detected_repo = get_git_remote_info()

    parser = argparse.ArgumentParser(description="Forgejo Issue Management Utility")
    parser.add_argument("--url", help="Forgejo server URL (default: auto-detected or http://192.168.2.2:3001)")
    parser.add_argument("--token", help="Forgejo Personal Access Token (or FORGEJO_TOKEN env var)")
    parser.add_argument("--owner", help="Repository owner (default: auto-detected from git remote)")
    parser.add_argument("--repo", help="Repository name (default: auto-detected from git remote)")

    subparsers = parser.add_subparsers(dest="command", required=True)

    # list
    list_parser = subparsers.add_parser("list", help="List open issues")
    list_parser.add_argument(
        "--kind",
        choices=VALID_KINDS,
        default="all",
        help="Filter by kind: bug, performance, data-integrity, security, all",
    )
    list_parser.add_argument(
        "--type",
        help="Alias for --kind",
        default=None,
    )
    list_parser.add_argument(
        "--severity",
        choices=VALID_SEVERITIES,
        default="all",
        help="Filter by severity: low, medium, high, all",
    )
    list_parser.add_argument("--limit", "-n", type=int, default=5, help="Maximum number of issues to list")
    list_parser.add_argument(
        "--include-solved",
        action="store_true",
        help="Include issues already tagged with status/solved",
    )

    # get
    get_parser = subparsers.add_parser("get", help="Get issue details and comments")
    get_parser.add_argument("issue_number", type=int, help="Issue number to fetch")

    # comment
    comment_parser = subparsers.add_parser("comment", help="Post a comment on an issue")
    comment_parser.add_argument("issue_number", type=int, help="Issue number")
    comment_parser.add_argument("body", help="Comment text to post")

    # solve (tags status/solved, does NOT close)
    solve_parser = subparsers.add_parser("solve", help="Mark an issue as solved with status/solved label (keeps issue open)")
    solve_parser.add_argument("issue_number", type=int, help="Issue number to mark as solved")
    solve_parser.add_argument("--comment", help="Optional resolution comment")

    # close (explicit close)
    close_parser = subparsers.add_parser("close", help="Close an issue")
    close_parser.add_argument("issue_number", type=int, help="Issue number to close")
    close_parser.add_argument("--comment", help="Optional closing comment")

    args = parser.parse_args()
    args.detected_host = detected_host
    args.detected_owner = detected_owner
    args.detected_repo = detected_repo

    if args.command == "list":
        cmd_list(args)
    elif args.command == "get":
        cmd_get(args)
    elif args.command == "comment":
        cmd_comment(args)
    elif args.command == "solve":
        cmd_solve(args)
    elif args.command == "close":
        cmd_close(args)


if __name__ == "__main__":
    main()
