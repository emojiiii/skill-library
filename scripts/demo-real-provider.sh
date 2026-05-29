#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/demo-real-provider.sh --dry-run
  scripts/demo-real-provider.sh --execute

Records evidence for the Team AI Hub real-provider MVP demo.

Required for --execute:
  TEAMAI_DEMO_WORKSPACE=OWNER/team-ai-hub-demo-skills
  GITHUB_TOKEN=...

Optional:
  TEAMAI_API=http://localhost:8787
  TEAMAI_DEMO_SKILL=code-reviewer
  TEAMAI_DEMO_REPO_DIR=./team-ai-hub-demo-skills
  TEAMAI_DEMO_LOCAL_SKILL=$HOME/.claude/skills/local-helper
  TEAMAI_DEMO_INVITEE=github-username
  TEAMAI_WEBHOOK_CALLBACK_URL=https://example.com/api/webhooks/github
  GITHUB_WEBHOOK_SECRET=local-dev-secret
  TEAMAI_EVIDENCE_DIR=.teamai-demo-evidence

Notes:
  --dry-run prints the command plan and writes no evidence.
  --execute runs real GitHub/API-affecting commands and stores logs under TEAMAI_EVIDENCE_DIR.
USAGE
}

if [[ $# -ne 1 ]]; then
  usage
  exit 2
fi

mode="$1"
if [[ "$mode" != "--dry-run" && "$mode" != "--execute" ]]; then
  usage
  exit 2
fi

workspace="${TEAMAI_DEMO_WORKSPACE:-}"
api="${TEAMAI_API:-http://localhost:8787}"
skill="${TEAMAI_DEMO_SKILL:-code-reviewer}"
repo_dir="${TEAMAI_DEMO_REPO_DIR:-team-ai-hub-demo-skills}"
local_skill="${TEAMAI_DEMO_LOCAL_SKILL:-$HOME/.claude/skills/local-helper}"
invitee="${TEAMAI_DEMO_INVITEE:-}"
webhook_url="${TEAMAI_WEBHOOK_CALLBACK_URL:-}"
webhook_secret="${GITHUB_WEBHOOK_SECRET:-}"
evidence_root="${TEAMAI_EVIDENCE_DIR:-.teamai-demo-evidence}"

if [[ "$mode" == "--execute" ]]; then
  if [[ -z "$workspace" ]]; then
    echo "TEAMAI_DEMO_WORKSPACE is required for --execute" >&2
    exit 2
  fi
  if [[ -z "${GITHUB_TOKEN:-}" ]]; then
    echo "GITHUB_TOKEN is required for --execute" >&2
    exit 2
  fi
fi

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
evidence_dir="$evidence_root/$timestamp"

redact() {
  sed \
    -e "s#${GITHUB_TOKEN:-__NO_TOKEN__}#[REDACTED_GITHUB_TOKEN]#g" \
    -e "s#${webhook_secret:-__NO_SECRET__}#[REDACTED_WEBHOOK_SECRET]#g"
}

print_cmd() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
}

run_step() {
  local name="$1"
  shift
  if [[ "$mode" == "--dry-run" ]]; then
    print_cmd "$@"
    return 0
  fi

  local log="$evidence_dir/${name}.log"
  {
    printf '$'
    printf ' %q' "$@"
    printf '\n\n'
    "$@"
  } > >(redact | tee "$log") 2> >(redact | tee -a "$log" >&2)
}

if [[ "$mode" == "--dry-run" ]]; then
  cat <<DRYRUN
Team AI Hub real-provider demo dry run

Environment used for planning:
  TEAMAI_DEMO_WORKSPACE=${workspace:-OWNER/team-ai-hub-demo-skills}
  TEAMAI_API=$api
  TEAMAI_DEMO_SKILL=$skill
  TEAMAI_DEMO_REPO_DIR=$repo_dir
  TEAMAI_DEMO_LOCAL_SKILL=$local_skill
  TEAMAI_DEMO_INVITEE=${invitee:-<skipped unless set>}
  TEAMAI_WEBHOOK_CALLBACK_URL=${webhook_url:-<skipped unless set>}
  GITHUB_WEBHOOK_SECRET=${webhook_secret:+[set]}

Command plan:
DRYRUN
else
  mkdir -p "$evidence_dir"
  {
    echo "# Team AI Hub Demo Evidence"
    echo
    echo "- Started: $timestamp"
    echo "- Workspace: $workspace"
    echo "- API: $api"
    echo "- Skill: $skill"
    echo "- Demo Repo Dir: $repo_dir"
    echo "- Local Skill: $local_skill"
    echo "- Invitee: ${invitee:-not provided}"
    echo
    echo "## Logs"
  } > "$evidence_dir/README.md"
fi

run_step "00-cargo-test" rtk cargo test --workspace
run_step "01-pnpm-check" rtk pnpm -r check
run_step "02-pnpm-test" rtk pnpm -r test
run_step "03-login" rtk cargo run -p teamai-cli -- login github --token "${GITHUB_TOKEN:-GITHUB_TOKEN}"
run_step "04-auth-status" rtk cargo run -p teamai-cli -- auth status

workspace_add=(rtk cargo run -p teamai-cli -- workspace add "${workspace:-OWNER/team-ai-hub-demo-skills}" --token "${GITHUB_TOKEN:-GITHUB_TOKEN}")
if [[ -n "$webhook_url" && -n "$webhook_secret" ]]; then
  workspace_add+=(--webhook-url "$webhook_url" --webhook-secret "$webhook_secret" --webhook-event push --webhook-event release)
fi
run_step "05-workspace-add" "${workspace_add[@]}"

run_step "06-scan-remote" rtk cargo run -p teamai-cli -- scan-remote "${workspace:-OWNER/team-ai-hub-demo-skills}" --token "${GITHUB_TOKEN:-GITHUB_TOKEN}"
run_step "07-subscribe" rtk cargo run -p teamai-cli -- subscribe "${workspace:-OWNER/team-ai-hub-demo-skills}" "$skill" --target claude-code --target cursor --target codex --update auto-patch
run_step "08-sync" rtk cargo run -p teamai-cli -- sync --token "${GITHUB_TOKEN:-GITHUB_TOKEN}" --pull-notifications --api "$api" --yes
run_step "09-status-after-sync" rtk cargo run -p teamai-cli -- status --target claude-code --target cursor --target codex
run_step "10-versions" rtk cargo run -p teamai-cli -- versions "${workspace:-OWNER/team-ai-hub-demo-skills}" --skill "$skill" --token "${GITHUB_TOKEN:-GITHUB_TOKEN}"
run_step "11-diff" rtk cargo run -p teamai-cli -- diff "${workspace:-OWNER/team-ai-hub-demo-skills}" v1.0.0 v1.2.0 --skill-path "$skill" --token "${GITHUB_TOKEN:-GITHUB_TOKEN}"
run_step "12-publish-pr" rtk cargo run -p teamai-cli -- package "$local_skill" --workspace "${workspace:-OWNER/team-ai-hub-demo-skills}" --publish-pr --auto-merge --token "${GITHUB_TOKEN:-GITHUB_TOKEN}" --api "$api" --yes

if [[ -n "$invitee" ]]; then
  run_step "13-invite" rtk cargo run -p teamai-cli -- invite "${workspace:-OWNER/team-ai-hub-demo-skills}" "$invitee" --role read --token "${GITHUB_TOKEN:-GITHUB_TOKEN}" --api "$api"
elif [[ "$mode" == "--dry-run" ]]; then
  echo "# invite step skipped unless TEAMAI_DEMO_INVITEE is set"
fi

run_step "14-push-update" rtk pnpm demo:push-update "$repo_dir"
run_step "15-notifications" rtk cargo run -p teamai-cli -- notifications --api "$api" --repository "${workspace:-OWNER/team-ai-hub-demo-skills}"
run_step "16-sync-after-update" rtk cargo run -p teamai-cli -- sync --token "${GITHUB_TOKEN:-GITHUB_TOKEN}" --pull-notifications --api "$api" --yes
run_step "17-status-after-update" rtk cargo run -p teamai-cli -- status --target claude-code --target cursor --target codex
run_step "18-rollback" rtk cargo run -p teamai-cli -- rollback "${workspace:-OWNER/team-ai-hub-demo-skills}" "$skill" v1.2.0 --target claude-code --target cursor --target codex --token "${GITHUB_TOKEN:-GITHUB_TOKEN}" --yes
run_step "19-status-after-rollback" rtk cargo run -p teamai-cli -- status --target claude-code --target cursor --target codex

if [[ "$mode" == "--execute" ]]; then
  cat >> "$evidence_dir/README.md" <<EOF
- [00-cargo-test.log](00-cargo-test.log)
- [01-pnpm-check.log](01-pnpm-check.log)
- [02-pnpm-test.log](02-pnpm-test.log)
- [03-login.log](03-login.log)
- [04-auth-status.log](04-auth-status.log)
- [05-workspace-add.log](05-workspace-add.log)
- [06-scan-remote.log](06-scan-remote.log)
- [07-subscribe.log](07-subscribe.log)
- [08-sync.log](08-sync.log)
- [09-status-after-sync.log](09-status-after-sync.log)
- [10-versions.log](10-versions.log)
- [11-diff.log](11-diff.log)
- [12-publish-pr.log](12-publish-pr.log)
EOF
  if [[ -n "$invitee" ]]; then
    echo "- [13-invite.log](13-invite.log)" >> "$evidence_dir/README.md"
  fi
  cat >> "$evidence_dir/README.md" <<EOF
- [14-push-update.log](14-push-update.log)
- [15-notifications.log](15-notifications.log)
- [16-sync-after-update.log](16-sync-after-update.log)
- [17-status-after-update.log](17-status-after-update.log)
- [18-rollback.log](18-rollback.log)
- [19-status-after-rollback.log](19-status-after-rollback.log)

## Manual Evidence Still Required

- \`workspace-skill.png\`: Desktop \`/\` screenshot with \`$skill\` selected and SKILL.md rendered.
- \`compare-diff.png\`: Desktop compare screenshot for \`v1.0.0\` to \`v1.2.0\`.
- \`publish-management.png\`: Desktop \`/publish\` screenshot showing request, policy, and merge state.
- \`invitations-management.png\`: Desktop \`/invitations\` screenshot showing invitation/onboarding state when applicable.

Run:

\`\`\`bash
rtk pnpm demo:verify-evidence "$evidence_dir"
\`\`\`
EOF
  echo "Evidence written to $evidence_dir"
fi
