#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/smoke-diagnostics.sh

Runs an isolated diagnostics export smoke and verifies that the generated
bundle contains local state summaries and redacted logs, but no credentials.
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

root="$(mktemp -d /private/tmp/teamai-diagnostics-smoke.XXXXXX)"
home="$root/home"
out="$root/diagnostics"
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
description: Reviews code changes.
version: 1.2.0
targets: [claude-code, cursor, codex]
permissions: [filesystem.read]
tags: [review]
---

# Code Reviewer
EOF

run_teamai() {
  HOME="$home" CARGO_HOME="$cargo_home" RUSTUP_HOME="$rustup_home" rtk cargo run -q -p teamai-cli -- "$@"
}

run_teamai init > "$root/00-init.log"
run_teamai subscribe acme/team-skills code-reviewer \
  --version 1.2.0 \
  --update manual \
  --target claude-code \
  --target cursor \
  --target codex > "$root/01-subscribe.log"
run_teamai sync \
  --source "$source" \
  --target-root "claude-code=$targets/claude-code" \
  --target-root "cursor=$targets/cursor" \
  --target-root "codex=$targets/codex" \
  --yes > "$root/02-sync.log"

echo 'token=ghp_abcdefghijklmnopqrstuvwxyz1234567890' >> "$home/.team-ai-hub/logs/test-token.log"
run_teamai diagnostics --output "$out" > "$root/03-diagnostics.log"

for file in diagnostics.json summary.json subscriptions.json workspaces.json; do
  if [[ ! -s "$out/$file" ]]; then
    echo "FAIL: diagnostics export missing $file" >&2
    exit 1
  fi
done

if [[ -e "$out/credentials.json" ]]; then
  echo "FAIL: diagnostics export must not include credentials.json" >&2
  exit 1
fi
if grep -R -Eq "ghp_[[:alnum:]_]+|github_pat_[[:alnum:]_]+|GITHUB_TOKEN" "$out"; then
  echo "FAIL: diagnostics export contains an unredacted token-looking value" >&2
  exit 1
fi
if ! grep -R -q "\[REDACTED\]" "$out/logs"; then
  echo "FAIL: diagnostics log copy did not redact token-looking values" >&2
  exit 1
fi

node - "$out/diagnostics.json" <<'NODE'
const fs = require("node:fs");
const diagnostics = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
if (diagnostics.subscriptions !== 1) {
  console.error("FAIL: expected one subscription in diagnostics");
  process.exit(1);
}
if (!Array.isArray(diagnostics.workspaces)) {
  console.error("FAIL: diagnostics workspaces should be an array");
  process.exit(1);
}
if (!Array.isArray(diagnostics.logs) || diagnostics.logs.length === 0) {
  console.error("FAIL: diagnostics should include copied log paths");
  process.exit(1);
}
if (!diagnostics.notes?.some((note) => note.includes("credentials"))) {
  console.error("FAIL: diagnostics notes should mention excluded credentials");
  process.exit(1);
}
NODE

cat <<EOF
Diagnostics smoke passed.

Evidence directory:
  $root

Diagnostics bundle:
  $out
EOF
