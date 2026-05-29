# Team AI Hub Quickstart

This guide runs the current MVP locally: Hono API, Tauri/Vite desktop UI, Rust CLI, routed management pages, and the demo Skill workspace.

## Prerequisites

- Rust stable toolchain with Cargo.
- Node.js 20+ and `pnpm` 9.
- Optional for GitHub-backed flows: a GitHub token or OAuth Device Flow client ID with `repo`, `read:org`, and `read:user` access.

## Local Development

1. Install dependencies:

   ```bash
   rtk pnpm install
   ```

2. Start Postgres for persistent API state:

   ```bash
   rtk docker compose up -d postgres
   ```

3. Copy `.env.example` into your shell or process manager. For local API state with Postgres:

   ```bash
   export DATABASE_URL=postgres://teamai:teamai@localhost:54329/teamai
   export GITHUB_WEBHOOK_SECRET=local-dev-secret
   export TEAMAI_WEBHOOK_CALLBACK_URL=https://example.com/api/webhooks/github
   ```

   Without `DATABASE_URL`, the API falls back to `.teamai-api-state.json`. Set `TEAMAI_API_STATE_PATH` to choose a different JSON file for demos.

4. Run the API:

   ```bash
   rtk pnpm dev:api
   ```

5. Run the desktop web preview:

   ```bash
   rtk pnpm dev:desktop
   ```

   Open `http://127.0.0.1:1420/`.

6. Run the Tauri desktop shell when native commands are needed:

   ```bash
   rtk pnpm --dir apps/desktop tauri dev
   ```

## CLI Build And Install

Build the Rust CLI:

```bash
rtk cargo build -p teamai-cli
```

Run it from the workspace:

```bash
rtk cargo run -p teamai-cli -- --help
```

Install the binary into Cargo's bin directory:

```bash
rtk cargo install --path crates/teamai-cli
```

Initialize local state and authenticate:

```bash
rtk teamai init
rtk teamai login github --client-id "$GITHUB_CLIENT_ID"
rtk teamai auth status
```

PAT fallback:

```bash
rtk teamai login github --token "$GITHUB_TOKEN"
```

## Demo Workspace

The browser preview has a built-in demo workspace at `acme/team-skills`, so the UI works without GitHub credentials.

For a real GitHub-backed demo repository, create a public or private repo with two directories:

```text
code-reviewer/
  SKILL.md
pr-summarizer/
  SKILL.md
README.md
```

You can generate that repository locally:

```bash
rtk pnpm demo:create-repo
```

Then push `./team-ai-hub-demo-skills` to GitHub as `owner/team-ai-hub-demo-skills`.

Each `SKILL.md` should include frontmatter like:

```markdown
---
schemaVersion: 1
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews code changes for correctness and security.
version: 1.2.0
targets: [claude-code, cursor, codex]
permissions: [filesystem.read, shell.execute.limited]
tags: [review, security]
---

# Code Reviewer

Run focused review passes over a local diff.
```

Tag at least three refs:

```bash
rtk git tag v1.0.0
rtk git tag v1.1.0
rtk git tag v1.2.0
rtk git push origin v1.0.0 v1.1.0 v1.2.0
```

Keep `v1.2.1` for the update step. The fixture helper can push it during the final demo:

```bash
export TEAMAI_DEMO_REPO_DIR=./team-ai-hub-demo-skills
rtk pnpm demo:push-update "$TEAMAI_DEMO_REPO_DIR"
```

Then add and scan it:

```bash
rtk teamai workspace add owner/team-ai-hub-demo-skills
rtk teamai scan-remote owner/team-ai-hub-demo-skills
```

## Routed Page Smoke Coverage

The desktop app exposes the MVP management pages through TanStack Router:

- `/` for workspace browsing and Skill detail.
- `/publish` for publish PR and policy-check management.
- `/invitations` for collaborator invitations and onboarding.
- `/subscriptions` for local subscription state.
- `/installed` for locally managed installs per Claude Code, Cursor, and Codex target.
- `/activity` for provider webhook/update notifications used by sync and daemon polling.
- `/cli` for CLI workflow pointers.

Run the committed route-contract tests:

```bash
rtk pnpm --dir apps/desktop test
```

Run a browser smoke check:

```bash
rtk pnpm --dir apps/desktop exec vite --host 127.0.0.1 --port 1421
```

Open:

```text
http://127.0.0.1:1421/
http://127.0.0.1:1421/publish
http://127.0.0.1:1421/invitations
```

Expected:

- Workspace route shows GitHub access, repository search, workspace scan, Skill list, README, and Skill detail.
- Publish route shows API base, active workspace, Open PRs, Merged, Review gates, Rejected checks, Publish requests, and Policy checks.
- Invitations route shows API base, active workspace, Pending, Accepted, Workspace ready, Invite collaborator, Invitations, and onboarding lookup.

Run the local CLI key-path smoke:

```bash
rtk pnpm smoke:cli-keypath
```

This creates an isolated temporary HOME, scans a local Skill, subscribes to it, sync-installs it into explicit Claude Code, Cursor, and Codex target roots, and verifies `teamai status` plus installed metadata. The final real-provider demo still covers GitHub login, workspace add, publish PR, invitation, notification, update, and rollback against a real repository.

CLI lifecycle logs are written to `~/.team-ai-hub/logs/YYYY-MM-DD.log`. Add `--verbose` to any `teamai` command to mirror the same logs to stderr without changing stdout output.

Export a sanitized diagnostics bundle:

```bash
rtk teamai diagnostics
```

The bundle includes local config, subscriptions, workspace lock summaries, and redacted log copies. It intentionally excludes `credentials.json` and OS keychain secrets. Run `rtk pnpm smoke:diagnostics` to verify this behavior in an isolated temporary HOME.

The desktop `/cli` page exposes the same diagnostics export plus a Logs button that opens `~/.team-ai-hub/logs` in the native Tauri shell.

Run the local rollback smoke:

```bash
rtk pnpm smoke:rollback
```

This uses the same isolated HOME approach to sync-install `v1.2.1` from a local source, roll back to `v1.2.0`, and verify all target installs plus the pinned lockfile state.

## Real-Provider Demo Evidence

The final MVP validation needs a real GitHub repository and token, not the built-in browser fallback data.

Print the real-provider command plan:

```bash
rtk pnpm demo:real-provider:dry-run
```

Execute and record CLI/API logs after setting `TEAMAI_DEMO_WORKSPACE` and `GITHUB_TOKEN`:

```bash
export TEAMAI_DEMO_WORKSPACE=owner/team-ai-hub-demo-skills
export TEAMAI_DEMO_REPO_DIR=./team-ai-hub-demo-skills
export GITHUB_TOKEN=...
rtk pnpm demo:real-provider
```

Logs are written under `.teamai-demo-evidence/<timestamp>/`. Use `docs/DEMO_RUNBOOK.md` for the manual screenshot checklist that completes the evidence set.

After adding the required screenshots, verify the folder:

```bash
rtk pnpm demo:verify-evidence .teamai-demo-evidence/<timestamp>
```

## Publish PR Flow

Preview a local Skill package:

```bash
rtk teamai package ~/.claude/skills/local-helper --workspace owner/team-ai-hub-demo-skills
```

Create a publish PR:

```bash
rtk teamai package ~/.claude/skills/local-helper \
  --workspace owner/team-ai-hub-demo-skills \
  --publish-pr \
  --token "$GITHUB_TOKEN" \
  --api http://localhost:8787
```

Low-risk publish packages can request auto-merge:

```bash
rtk teamai package ~/.claude/skills/local-helper \
  --workspace owner/team-ai-hub-demo-skills \
  --publish-pr \
  --auto-merge \
  --token "$GITHUB_TOKEN" \
  --api http://localhost:8787 \
  --yes
```

The API records publish requests and policy checks. The desktop `/publish` page displays both.

## Invite And Onboard

Invite a collaborator:

```bash
rtk teamai invite owner/team-ai-hub-demo-skills octocat \
  --role read \
  --token "$GITHUB_TOKEN" \
  --api http://localhost:8787
```

Use the desktop `/invitations` page to view pending invitations, run onboarding lookup, and accept demo onboarding invitations.

## Sync, Status, Diff, Rollback

Subscribe and sync:

```bash
rtk teamai subscribe owner/team-ai-hub-demo-skills code-reviewer \
  --target claude-code \
  --target cursor \
  --target codex \
  --update auto-patch
rtk teamai sync --token "$GITHUB_TOKEN" --pull-notifications --api http://localhost:8787
```

Run one daemon poll for local verification, or omit `--once` to keep polling:

```bash
rtk teamai daemon --once --interval-seconds 60 --api http://localhost:8787
```

Inspect install state:

```bash
rtk teamai status --target claude-code --target cursor --target codex
rtk teamai versions owner/team-ai-hub-demo-skills --skill code-reviewer --token "$GITHUB_TOKEN"
rtk teamai diff owner/team-ai-hub-demo-skills v1.0.0 v1.2.0 --skill-path code-reviewer --token "$GITHUB_TOKEN"
```

Rollback:

```bash
rtk teamai rollback owner/team-ai-hub-demo-skills code-reviewer v1.1.0 \
  --target claude-code \
  --target cursor \
  --target codex \
  --token "$GITHUB_TOKEN" \
  --yes
```

## Packaging And Self-Hosting

Build all Rust crates and TypeScript apps locally:

```bash
rtk cargo test --workspace
rtk pnpm -r check
rtk pnpm -r test
rtk pnpm --dir apps/api build
rtk pnpm --dir apps/desktop build
```

Build the Tauri desktop package:

```bash
rtk pnpm --dir apps/desktop tauri build
```

Self-host the API, web preview, and Postgres with Docker Compose:

```bash
rtk docker compose up --build
```

Open `http://localhost:8080/` for the web preview and `http://localhost:8787/health` for the API health check. Compose also exposes Postgres on `localhost:54329` for local inspection.

Minimum environment:

```text
GITHUB_CLIENT_ID=
GITHUB_CLIENT_SECRET=
GITHUB_WEBHOOK_SECRET=
TEAMAI_WEBHOOK_CALLBACK_URL=https://your-host.example/api/webhooks/github
```

For production, replace the Postgres password, serve the web/API endpoints behind HTTPS, and set `TEAMAI_WEBHOOK_CALLBACK_URL` to the public webhook endpoint registered with GitHub. For local API-only development, `rtk docker compose up -d postgres` plus `rtk pnpm dev:api` still works.
