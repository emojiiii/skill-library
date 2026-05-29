#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/create-demo-skills-repo.sh [--force] [DEST]

Creates a local Git repository fixture for the Team AI Hub real-provider demo.

Default DEST:
  ./team-ai-hub-demo-skills

The repo contains:
  README.md
  code-reviewer/SKILL.md
  pr-summarizer/SKILL.md

Tags:
  v1.0.0
  v1.1.0
  v1.2.0

The v1.2.1 update is intentionally not created by this script. Use
scripts/push-demo-update.sh during the final demo to push v1.2.1 and trigger
the webhook/update evidence step.

Use this fixture as the source for a GitHub repo named team-ai-hub-demo-skills.
USAGE
}

force=0
dest="team-ai-hub-demo-skills"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --force)
      force=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      dest="$1"
      shift
      ;;
  esac
done

if [[ -e "$dest" ]]; then
  if [[ "$force" -ne 1 ]]; then
    echo "Destination exists: $dest" >&2
    echo "Pass --force to replace it." >&2
    exit 2
  fi
  rm -rf "$dest"
fi

mkdir -p "$dest/code-reviewer" "$dest/pr-summarizer"
cd "$dest"

git init -q
git config user.name "Team AI Hub Demo"
git config user.email "demo@team-ai-hub.local"

cat > README.md <<'EOF'
# Team AI Hub Demo Skills

Demo repository for Team AI Hub MVP validation. It contains two Skills and a tag history designed to exercise browsing, semantic diff, subscription, sync, publish, invitation, update, and rollback flows.
EOF

cat > code-reviewer/SKILL.md <<'EOF'
---
schemaVersion: 1
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews code changes for correctness and security.
version: 1.0.0
targets: [claude-code, cursor, codex]
permissions: [filesystem.read]
tags: [review, security]
---

# Code Reviewer

Run focused review passes over a local diff.

## Workflow

1. Inspect changed files.
2. Flag defects before style notes.
EOF

cat > pr-summarizer/SKILL.md <<'EOF'
---
schemaVersion: 1
id: pr-summarizer
type: skill
name: PR Summarizer
description: Summarizes pull requests into reviewer-ready notes.
version: 1.0.0
targets: [claude-code, cursor]
permissions: [filesystem.read]
tags: [pull-request]
---

# PR Summarizer

Turns a pull request into a short reviewer briefing.
EOF

git add README.md code-reviewer/SKILL.md pr-summarizer/SKILL.md
git commit -q -m "Demo skills v1.0.0"
git tag v1.0.0

perl -0pi -e 's/version: 1\.0\.0/version: 1.1.0/' code-reviewer/SKILL.md pr-summarizer/SKILL.md
perl -0pi -e 's/1\. Inspect changed files\./1. Inspect changed files and nearby tests./' code-reviewer/SKILL.md
git add code-reviewer/SKILL.md pr-summarizer/SKILL.md
git commit -q -m "Demo skills v1.1.0"
git tag v1.1.0

perl -0pi -e 's/version: 1\.1\.0/version: 1.2.0/' code-reviewer/SKILL.md pr-summarizer/SKILL.md
perl -0pi -e 's/permissions: \[filesystem\.read\]/permissions: [filesystem.read, shell.execute.limited]/' code-reviewer/SKILL.md
perl -0pi -e 's/2\. Flag defects before style notes\./2. Flag defects before style notes.\n3. Summarize risk by file and severity./' code-reviewer/SKILL.md
git add code-reviewer/SKILL.md pr-summarizer/SKILL.md
git commit -q -m "Demo skills v1.2.0"
git tag v1.2.0

git branch -M main

cat <<EOF
Created demo repository: $PWD

Push it to GitHub:
  cd "$PWD"
  git remote add origin git@github.com:OWNER/team-ai-hub-demo-skills.git
  git push -u origin main
  git push origin v1.0.0 v1.1.0 v1.2.0

During the update step of the final demo:
  rtk pnpm demo:push-update "$PWD"

Then run:
  export TEAMAI_DEMO_WORKSPACE=OWNER/team-ai-hub-demo-skills
  export TEAMAI_DEMO_REPO_DIR="$PWD"
  rtk pnpm demo:real-provider:dry-run
EOF
