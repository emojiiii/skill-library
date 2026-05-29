#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/verify-demo-evidence.sh [--allow-missing-screenshots] EVIDENCE_DIR

Verifies that a real-provider demo evidence directory contains the logs and
manual screenshots required by docs/DEMO_RUNBOOK.md.

Expected screenshot filenames:
  workspace-skill.png
  compare-diff.png
  publish-management.png
  invitations-management.png

The verifier checks logs for command output signals, not just file existence.
USAGE
}

allow_missing_screenshots=0
evidence_dir=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --allow-missing-screenshots)
      allow_missing_screenshots=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      evidence_dir="$1"
      shift
      ;;
  esac
done

if [[ -z "$evidence_dir" ]]; then
  usage
  exit 2
fi

failures=0

fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}

pass() {
  echo "PASS: $*"
}

require_file() {
  local file="$1"
  if [[ -s "$evidence_dir/$file" ]]; then
    pass "$file exists"
  else
    fail "$file is missing or empty"
  fi
}

require_pattern() {
  local file="$1"
  local pattern="$2"
  local description="$3"
  if [[ ! -s "$evidence_dir/$file" ]]; then
    fail "$file is missing or empty; cannot check $description"
    return
  fi
  if grep -Eiq "$pattern" "$evidence_dir/$file"; then
    pass "$description"
  else
    fail "$description not found in $file"
  fi
}

require_absent() {
  local file="$1"
  local pattern="$2"
  local description="$3"
  if [[ ! -s "$evidence_dir/$file" ]]; then
    return
  fi
  if grep -Eiq "$pattern" "$evidence_dir/$file"; then
    fail "$description found in $file"
  else
    pass "$description absent from $file"
  fi
}

if [[ ! -d "$evidence_dir" ]]; then
  echo "Evidence directory does not exist: $evidence_dir" >&2
  exit 2
fi

required_logs=(
  "00-cargo-test.log"
  "01-pnpm-check.log"
  "02-pnpm-test.log"
  "03-login.log"
  "04-auth-status.log"
  "05-workspace-add.log"
  "06-scan-remote.log"
  "07-subscribe.log"
  "08-sync.log"
  "09-status-after-sync.log"
  "10-versions.log"
  "11-diff.log"
  "12-publish-pr.log"
  "14-push-update.log"
  "15-notifications.log"
  "16-sync-after-update.log"
  "17-status-after-update.log"
  "18-rollback.log"
  "19-status-after-rollback.log"
)

for log in "${required_logs[@]}"; do
  require_file "$log"
  require_absent "$log" "GITHUB_TOKEN|ghp_[[:alnum:]_]+|github_pat_[[:alnum:]_]+" "unredacted GitHub token"
done

require_pattern "00-cargo-test.log" "test result: ok|[0-9]+ passed" "cargo tests passed"
require_pattern "01-pnpm-check.log" "check" "pnpm check ran"
require_pattern "02-pnpm-test.log" "passed|Test Files.*passed" "pnpm tests passed"
require_pattern "03-login.log" "logged in|github|credential|@|user" "GitHub login completed"
require_pattern "04-auth-status.log" "github|credential|login|scopes" "auth status captured"
require_pattern "05-workspace-add.log" "team-ai-hub-demo-skills|workspace|webhook|added|saved" "workspace add captured"
require_pattern "06-scan-remote.log" "code-reviewer|pr-summarizer|skill" "remote scan found demo skills"
require_pattern "07-subscribe.log" "code-reviewer|subscribed|subscription" "subscription captured"
require_pattern "08-sync.log" "installed|sync|code-reviewer|lock" "sync captured"
require_pattern "09-status-after-sync.log" "claude-code|cursor|codex|code-reviewer|installed" "post-sync target status captured"
require_pattern "10-versions.log" "v1\\.0\\.0|v1\\.1\\.0|v1\\.2\\.0" "versions output includes demo tags"
require_pattern "11-diff.log" "shell\\.execute\\.limited|permissions|code-reviewer" "diff shows permission semantic change"
require_pattern "12-publish-pr.log" "pull|PR|publish|policy|auto" "publish PR output captured"
require_pattern "14-push-update.log" "v1\\.2\\.1|push|Pushed demo update" "demo update push captured"
require_pattern "15-notifications.log" "notification|workspace_updated|team-ai-hub-demo-skills|v1\\.2\\.1" "notification/update evidence captured"
require_pattern "16-sync-after-update.log" "notifications|sync|v1\\.2\\.1|code-reviewer|installed" "post-update sync captured"
require_pattern "17-status-after-update.log" "claude-code|cursor|codex|v1\\.2\\.1|code-reviewer|installed" "post-update target status captured"
require_pattern "18-rollback.log" "rollback|v1\\.2\\.0|code-reviewer" "rollback output captured"
require_pattern "19-status-after-rollback.log" "claude-code|cursor|codex|v1\\.2\\.0|pinned|code-reviewer" "post-rollback target status captured"

if compgen -G "$evidence_dir/13-invite.log" > /dev/null; then
  require_pattern "13-invite.log" "invite|pending|accepted|collaborator|invitation" "invitation output captured"
else
  fail "13-invite.log is missing; set TEAMAI_DEMO_INVITEE for the final demo run"
fi

screenshots=(
  "workspace-skill.png"
  "compare-diff.png"
  "publish-management.png"
  "invitations-management.png"
)

for screenshot in "${screenshots[@]}"; do
  if [[ -s "$evidence_dir/$screenshot" ]]; then
    pass "$screenshot exists"
  elif [[ "$allow_missing_screenshots" -eq 1 ]]; then
    echo "WARN: $screenshot missing"
  else
    fail "$screenshot is missing"
  fi
done

if [[ "$failures" -gt 0 ]]; then
  echo
  echo "Evidence verification failed with $failures issue(s)." >&2
  exit 1
fi

echo
echo "Evidence verification passed."
