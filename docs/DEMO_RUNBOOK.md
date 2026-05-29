# Team AI Hub Real-Provider Demo Runbook

Use this runbook to execute and record the final MVP demo against a real GitHub repository named `team-ai-hub-demo-skills`.

The command sequence below is also captured in `scripts/demo-real-provider.sh`. Run `rtk pnpm demo:real-provider:dry-run` to print the planned commands without touching GitHub, or `rtk pnpm demo:real-provider` after setting the required environment variables to execute and record logs.

## Required Inputs

- GitHub repository: `OWNER/team-ai-hub-demo-skills`.
- GitHub token or Device Flow client ID with access to the repository.
- Demo local Skill source: `~/.claude/skills/local-helper`.
- Optional public webhook URL, for example a tunnel URL ending in `/api/webhooks/github`.
- Local agent targets installed or represented by explicit `--target-root` paths for Claude Code, Cursor, and Codex.

## Preflight

```bash
rtk cargo test --workspace
rtk pnpm -r check
rtk pnpm -r test
```

Start the database and API:

```bash
rtk docker compose up -d postgres
export DATABASE_URL=postgres://teamai:teamai@localhost:54329/teamai
export GITHUB_WEBHOOK_SECRET=local-dev-secret
export TEAMAI_WEBHOOK_CALLBACK_URL=https://YOUR-TUNNEL.example/api/webhooks/github
rtk pnpm dev:api
```

Start the desktop app:

```bash
rtk pnpm dev:desktop
```

## Demo Repository Shape

Create the local fixture:

```bash
rtk pnpm demo:create-repo
```

By default this creates `./team-ai-hub-demo-skills`. Push that repository to GitHub as `OWNER/team-ai-hub-demo-skills` before running the real-provider script.

The repository must contain:

```text
README.md
code-reviewer/SKILL.md
pr-summarizer/SKILL.md
```

Required tags:

```text
v1.0.0
v1.1.0
v1.2.0
```

`code-reviewer/SKILL.md` should add or change `shell.execute.limited` between `v1.0.0` and `v1.2.0` so the compare view has a meaningful semantic permission change. Keep `v1.2.1` unpushed until the update step.

## Script

Set these once:

```bash
export TEAMAI_DEMO_WORKSPACE=OWNER/team-ai-hub-demo-skills
export GITHUB_TOKEN=...
export TEAMAI_API=http://localhost:8787
```

Preview the command plan:

```bash
rtk pnpm demo:real-provider:dry-run
```

Execute the CLI/API evidence run:

```bash
rtk pnpm demo:real-provider
```

1. Login:

   ```bash
   rtk cargo run -p teamai-cli -- login github --token "$GITHUB_TOKEN"
   rtk cargo run -p teamai-cli -- auth status
   ```

2. Add and scan the workspace:

   ```bash
   rtk cargo run -p teamai-cli -- workspace add "$TEAMAI_DEMO_WORKSPACE" \
     --token "$GITHUB_TOKEN" \
     --webhook-url "$TEAMAI_WEBHOOK_CALLBACK_URL" \
     --webhook-secret "$GITHUB_WEBHOOK_SECRET" \
     --webhook-event push \
     --webhook-event release
   rtk cargo run -p teamai-cli -- scan-remote "$TEAMAI_DEMO_WORKSPACE" --token "$GITHUB_TOKEN"
   ```

3. Desktop route checks:

   - Open `/`.
   - Select `code-reviewer`.
   - Confirm SKILL.md renders.
   - Compare `v1.0.0` to `v1.2.0`.
   - Open `/publish`.
   - Open `/invitations`.

4. Subscribe and sync:

   ```bash
   rtk cargo run -p teamai-cli -- subscribe "$TEAMAI_DEMO_WORKSPACE" code-reviewer \
     --target claude-code \
     --target cursor \
     --target codex \
     --update auto-patch
   rtk cargo run -p teamai-cli -- sync \
     --token "$GITHUB_TOKEN" \
     --pull-notifications \
     --api "$TEAMAI_API" \
     --yes
   rtk cargo run -p teamai-cli -- status \
     --target claude-code \
     --target cursor \
     --target codex
   ```

5. Publish local Skill:

   ```bash
   rtk cargo run -p teamai-cli -- package ~/.claude/skills/local-helper \
     --workspace "$TEAMAI_DEMO_WORKSPACE" \
     --publish-pr \
     --auto-merge \
     --token "$GITHUB_TOKEN" \
     --api "$TEAMAI_API" \
     --yes
   ```

   Confirm `/publish` shows the publish request, policy result, and auto-merge state.

6. Invite a member:

   ```bash
   rtk cargo run -p teamai-cli -- invite "$TEAMAI_DEMO_WORKSPACE" GITHUB_USERNAME \
     --role read \
     --token "$GITHUB_TOKEN" \
     --api "$TEAMAI_API"
   ```

   Confirm `/invitations` shows pending invitation state.

7. Update:

   Push the `v1.2.1` demo update, then pull notifications and sync:

   ```bash
   rtk pnpm demo:push-update "$TEAMAI_DEMO_REPO_DIR"
   rtk cargo run -p teamai-cli -- notifications --api "$TEAMAI_API" --repository "$TEAMAI_DEMO_WORKSPACE"
   rtk cargo run -p teamai-cli -- sync \
     --token "$GITHUB_TOKEN" \
     --pull-notifications \
     --api "$TEAMAI_API" \
     --yes
   rtk cargo run -p teamai-cli -- status \
     --target claude-code \
     --target cursor \
     --target codex
   ```

8. Rollback:

   ```bash
   rtk cargo run -p teamai-cli -- rollback "$TEAMAI_DEMO_WORKSPACE" code-reviewer v1.2.0 \
     --target claude-code \
     --target cursor \
     --target codex \
     --token "$GITHUB_TOKEN" \
     --yes
   rtk cargo run -p teamai-cli -- status \
     --target claude-code \
     --target cursor \
     --target codex
   ```

## Evidence Checklist

Capture these artifacts before marking the MVP demo complete:

- `.teamai-demo-evidence/<timestamp>/00-cargo-test.log`, `01-pnpm-check.log`, and `02-pnpm-test.log` showing all checks passed.
- Desktop `/` screenshot saved as `workspace-skill.png` with `code-reviewer` selected and SKILL.md rendered.
- Desktop compare screenshot saved as `compare-diff.png` showing the permission semantic diff.
- Publish PR URL in `12-publish-pr.log` and `/publish` screenshot saved as `publish-management.png` showing request, policy, and merge state.
- Invitation CLI/API output in `13-invite.log` and `/invitations` screenshot saved as `invitations-management.png` showing pending state.
- `09-status-after-sync.log` from `teamai status --target claude-code --target cursor --target codex` after sync.
- `14-push-update.log`, `15-notifications.log`, `16-sync-after-update.log`, and `17-status-after-update.log` showing `v1.2.1` was pushed, received, synced, and installed.
- `19-status-after-rollback.log` after rollback showing the pinned/restored version.

Verify the captured folder before declaring the run complete:

```bash
rtk pnpm demo:verify-evidence .teamai-demo-evidence/<timestamp>
```

## Completion Gate

The real-provider demo is complete only when every checklist item has current evidence from the same repository and same run. Demo fallback data from the browser preview is not sufficient for this gate.
