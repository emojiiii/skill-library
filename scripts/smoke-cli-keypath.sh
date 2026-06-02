#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/smoke-cli-keypath.sh

Runs the local CLI key-path smoke required by the MVP docs:
  init -> auth status -> scan local Skill -> subscribe -> sync -> status

The smoke uses an isolated temporary HOME and explicit target roots so it does
not touch the caller's real Skill Library, Claude Code, Cursor, or Codex state.
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi
if [[ $# -ne 0 ]]; then
  usage >&2
  exit 2
fi

root="$(mktemp -d /private/tmp/skill-library-cli-smoke.XXXXXX)"
home="$root/home"
workspace="$root/workspace"
source="$workspace/code-reviewer"
targets="$root/targets"
real_home="${HOME:?}"
cargo_home="${CARGO_HOME:-$real_home/.cargo}"
rustup_home="${RUSTUP_HOME:-$real_home/.rustup}"
mkdir -p "$home" "$source" "$targets/claude-code" "$targets/cursor" "$targets/codex"

cat > "$source/SKILL.md" <<'EOF'
---
schemaVersion: 1
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews code changes for correctness and security.
version: 1.2.0
targets: [claude-code, cursor, codex]
permissions: [filesystem.read]
tags: [review, security]
---

# Code Reviewer

Run focused review passes over a local diff.
EOF

run_skill_library() {
  HOME="$home" CARGO_HOME="$cargo_home" RUSTUP_HOME="$rustup_home" rtk cargo run -q -p skill-library-cli -- "$@"
}

run_skill_library init > "$root/00-init.log"
run_skill_library auth status > "$root/01-auth-status.log"
run_skill_library scan "$workspace" > "$root/02-scan.log"
run_skill_library subscribe acme/team-skills code-reviewer \
  --version 1.2.0 \
  --update manual \
  --target claude-code \
  --target cursor \
  --target codex > "$root/03-subscribe.log"
run_skill_library sync \
  --source "$source" \
  --target-root "claude-code=$targets/claude-code" \
  --target-root "cursor=$targets/cursor" \
  --target-root "codex=$targets/codex" \
  --yes > "$root/04-sync.log"
run_skill_library status \
  --target claude-code \
  --target cursor \
  --target codex \
  --target-root "claude-code=$targets/claude-code" \
  --target-root "cursor=$targets/cursor" \
  --target-root "codex=$targets/codex" > "$root/05-status.log"

for target in claude-code cursor codex; do
  install_dir="$targets/$target/code-reviewer"
  if [[ ! -s "$install_dir/SKILL.md" ]]; then
    echo "FAIL: $target install is missing SKILL.md at $install_dir" >&2
    exit 1
  fi
  if [[ ! -s "$install_dir/.skill-library-install.json" ]]; then
    echo "FAIL: $target install is missing metadata at $install_dir" >&2
    exit 1
  fi
done

for log in 02-scan.log 03-subscribe.log 04-sync.log 05-status.log; do
  if ! grep -q "code-reviewer" "$root/$log"; then
    echo "FAIL: expected code-reviewer in $log" >&2
    exit 1
  fi
done

node - "$root/05-status.log" <<'NODE'
const fs = require("node:fs");
const status = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
for (const target of ["claude-code", "cursor", "codex"]) {
  const installed = status.installed?.[target] ?? [];
  const skill = installed.find((item) => item.id === "code-reviewer");
  if (!skill || skill.version !== "1.2.0" || skill.managed_by !== "skill-library") {
    console.error(`FAIL: ${target} status is missing code-reviewer v1.2.0 metadata`);
    process.exit(1);
  }
}
const locked = status.locks?.[0]?.lock?.assets?.find((item) => item.asset_id === "code-reviewer");
if (!locked || locked.version !== "1.2.0" || locked.ref_name !== "v1.2.0") {
  console.error("FAIL: status lock is missing code-reviewer v1.2.0");
  process.exit(1);
}
NODE

if ! grep -q "github: not logged in" "$root/01-auth-status.log"; then
  echo "FAIL: auth status did not report isolated not-logged-in state" >&2
  exit 1
fi

cli_log="$(find "$home/.skill-library/logs" -type f -name '*.log' -print | head -n 1)"
if [[ -z "$cli_log" || ! -s "$cli_log" ]]; then
  echo "FAIL: CLI did not write a log file under isolated ~/.skill-library/logs" >&2
  exit 1
fi
if ! grep -q "skill-library command started" "$cli_log"; then
  echo "FAIL: CLI log is missing command lifecycle entries" >&2
  exit 1
fi
if grep -Eq "GITHUB_TOKEN|ghp_[[:alnum:]_]+|github_pat_[[:alnum:]_]+" "$cli_log"; then
  echo "FAIL: CLI log contains an unredacted GitHub token-looking value" >&2
  exit 1
fi

cat <<EOF
CLI key-path smoke passed.

Evidence directory:
  $root

CLI log:
  $cli_log

Installed targets:
  $targets/claude-code/code-reviewer
  $targets/cursor/code-reviewer
  $targets/codex/code-reviewer
EOF
