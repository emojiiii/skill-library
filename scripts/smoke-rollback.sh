#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/smoke-rollback.sh

Runs an offline rollback smoke:
  init -> subscribe -> sync v1.2.1 from local source -> rollback to v1.2.0 from local source -> status

The smoke uses an isolated temporary HOME and explicit target roots so it does
not touch the caller's real Team AI Hub, Claude Code, Cursor, or Codex state.
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

root="$(mktemp -d /private/tmp/teamai-rollback-smoke.XXXXXX)"
home="$root/home"
workspace="$root/workspace"
source_v121="$workspace/v1.2.1/code-reviewer"
source_v120="$workspace/v1.2.0/code-reviewer"
targets="$root/targets"
real_home="${HOME:?}"
cargo_home="${CARGO_HOME:-$real_home/.cargo}"
rustup_home="${RUSTUP_HOME:-$real_home/.rustup}"
mkdir -p "$home" "$source_v121" "$source_v120" "$targets/claude-code" "$targets/cursor" "$targets/codex"

write_skill() {
  local dir="$1"
  local version="$2"
  local note="$3"
  cat > "$dir/SKILL.md" <<EOF
---
schemaVersion: 1
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews code changes for correctness and security.
version: $version
targets: [claude-code, cursor, codex]
permissions: [filesystem.read]
tags: [review, security]
---

# Code Reviewer

$note
EOF
}

write_skill "$source_v121" "1.2.1" "Current patch release."
write_skill "$source_v120" "1.2.0" "Rollback target release."

run_teamai() {
  HOME="$home" CARGO_HOME="$cargo_home" RUSTUP_HOME="$rustup_home" rtk cargo run -q -p teamai-cli -- "$@"
}

run_teamai init > "$root/00-init.log"
run_teamai subscribe acme/team-skills code-reviewer \
  --version 1.2.1 \
  --update auto-patch \
  --target claude-code \
  --target cursor \
  --target codex > "$root/01-subscribe.log"
run_teamai sync \
  --source "$source_v121" \
  --target-root "claude-code=$targets/claude-code" \
  --target-root "cursor=$targets/cursor" \
  --target-root "codex=$targets/codex" \
  --yes > "$root/02-sync-v121.log"
run_teamai rollback acme/team-skills code-reviewer v1.2.0 \
  --source "$source_v120" \
  --target claude-code \
  --target cursor \
  --target codex \
  --target-root "claude-code=$targets/claude-code" \
  --target-root "cursor=$targets/cursor" \
  --target-root "codex=$targets/codex" \
  --yes > "$root/03-rollback-v120.log"
run_teamai status \
  --target claude-code \
  --target cursor \
  --target codex \
  --target-root "claude-code=$targets/claude-code" \
  --target-root "cursor=$targets/cursor" \
  --target-root "codex=$targets/codex" > "$root/04-status.log"

for target in claude-code cursor codex; do
  install_dir="$targets/$target/code-reviewer"
  if [[ ! -s "$install_dir/SKILL.md" ]]; then
    echo "FAIL: $target rollback install is missing SKILL.md at $install_dir" >&2
    exit 1
  fi
  if ! grep -q "version: 1.2.0" "$install_dir/SKILL.md"; then
    echo "FAIL: $target rollback install is not v1.2.0" >&2
    exit 1
  fi
done

node - "$root/04-status.log" <<'NODE'
const fs = require("node:fs");
const status = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
for (const target of ["claude-code", "cursor", "codex"]) {
  const installed = status.installed?.[target] ?? [];
  const skill = installed.find((item) => item.id === "code-reviewer");
  if (!skill || skill.version !== "1.2.0" || skill.managed_by !== "team-ai-hub") {
    console.error(`FAIL: ${target} status is missing rollback code-reviewer v1.2.0 metadata`);
    process.exit(1);
  }
}
const locked = status.locks?.[0]?.lock?.assets?.find((item) => item.asset_id === "code-reviewer");
if (!locked || locked.version !== "v1.2.0" && locked.version !== "1.2.0") {
  console.error("FAIL: status lock is missing rollback code-reviewer v1.2.0");
  process.exit(1);
}
if (locked.ref_name !== "v1.2.0" || locked.pinned !== true) {
  console.error("FAIL: rollback lock is not pinned to ref v1.2.0");
  process.exit(1);
}
NODE

if ! grep -q "rollback installed the requested version" "$root/03-rollback-v120.log"; then
  echo "FAIL: rollback output did not include completion note" >&2
  exit 1
fi

cat <<EOF
Rollback smoke passed.

Evidence directory:
  $root

Installed targets:
  $targets/claude-code/code-reviewer
  $targets/cursor/code-reviewer
  $targets/codex/code-reviewer
EOF
