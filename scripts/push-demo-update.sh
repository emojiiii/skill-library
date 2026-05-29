#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/push-demo-update.sh [REPO_DIR]

Creates and pushes the Team AI Hub demo update tag v1.2.1.

Default REPO_DIR:
  ./team-ai-hub-demo-skills

This script is intended for the final real-provider demo after the initial
repository with v1.0.0, v1.1.0, and v1.2.0 has already been pushed.
USAGE
}

repo_dir="team-ai-hub-demo-skills"

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    *)
      repo_dir="$1"
      shift
      ;;
  esac
done

if [[ ! -d "$repo_dir/.git" ]]; then
  echo "Not a Git repository: $repo_dir" >&2
  exit 2
fi

cd "$repo_dir"

if git rev-parse -q --verify refs/tags/v1.2.1 >/dev/null; then
  echo "Tag v1.2.1 already exists locally." >&2
else
  git checkout -q main
  perl -0pi -e 's/version: 1\.2\.0/version: 1.2.1/' code-reviewer/SKILL.md pr-summarizer/SKILL.md
  perl -0pi -e 's/description: Reviews code changes for correctness and security\./description: Reviews code changes for correctness, security, and rollback risk./' code-reviewer/SKILL.md
  git add code-reviewer/SKILL.md pr-summarizer/SKILL.md
  git commit -q -m "Demo skills v1.2.1"
  git tag v1.2.1
fi

git push origin main
git push origin v1.2.1

cat <<EOF
Pushed demo update from: $PWD

Evidence step:
  rtk cargo run -p teamai-cli -- notifications --repository OWNER/team-ai-hub-demo-skills
  rtk cargo run -p teamai-cli -- sync --pull-notifications --yes
EOF
