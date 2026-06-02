# Skill Library

Skill Library is a Git-provider-backed workflow layer for team AI Skills.

This repository currently contains the MVP scaffold described in `docs/`:

- Tauri v2 + React desktop app.
- Rust core crates for manifests, providers, installer, sync, publish, and CLI.
- Hono API control-plane scaffold.
- Local-first subscription and install primitives.

See `QUICKSTART.md` for local development, CLI install, demo workspace setup, routed-page smoke coverage, Docker Compose self-hosting, and packaging commands. Use `rtk pnpm demo:create-repo`, `rtk pnpm demo:push-update`, `docs/DEMO_RUNBOOK.md`, and `rtk pnpm demo:real-provider:dry-run` for the final real-provider demo evidence checklist.

## Current MVP Capabilities

- Parse local skills from `manifest.yaml`, `manifest.json`, or `SKILL.md` frontmatter.
- Connect GitHub workspaces with GitHub Device Flow or a token fallback, storing provider tokens in the OS keychain.
- Browse GitHub repositories, workspace skills, workspace README, skill source, and git tags in the desktop workbench.
- Subscribe and install local skills into Claude Code, Cursor, and Codex target directories.
- Download GitHub source archives for subscribed skills, extract them into the workspace cache, install the matching skill, and write workspace lockfiles through `skill-library sync`.
- Roll back a skill to an older version by downloading or using a provided source payload, reinstalling it, and pinning the lockfile.
- Restore the previous installed skill directory if an update swap fails, and keep sync lockfiles pinned to the last working version when install fails.
- Preview publish PR metadata for a local skill package.
- Create a GitHub publish branch, upload local skill files, and open a pull request with `skill-library package --publish-pr` after checking the current user's write permission.
- Require explicit confirmation for medium-or-higher risk installs, sync updates, rollbacks, and publish PR creation; CLI supports interactive confirmation or `--yes`, and the desktop workbench requires a confirmation step before risky installs.
- Evaluate publish policy for schema-pass packages, dangerous permissions, scripts, and large files; publish PR output includes policy results, and `skill-library package --publish-pr --auto-merge` only attempts merge when policy and maintainer permissions allow it.
- Invite GitHub collaborators from the CLI, Tauri command layer, and desktop workbench after confirming the current user has maintain/admin permission; the desktop invitation center also lists current GitHub collaborators and roles.
- Register GitHub push webhooks when adding a workspace with webhook callback/secret settings, and persist the provider hook handle in the workspace registry.
- Verify GitHub webhook signatures in the Hono API before accepting push payloads.
- Persist accepted GitHub push/release webhook events into the API state store, pull them with `skill-library notifications`, and include them in `skill-library sync --pull-notifications` / `skill-library daemon` polling cycles.
- Persist publish request, invitation, and notification management records through the Hono API state store; `DATABASE_URL` uses Postgres, while local demos can fall back to `.skill-library-api-state.json`.
- Persist cloud publish policy check results through the same API state store and surface them in the routed desktop publish management page.
- Expose invited-user onboarding through API invitation landing/accept endpoints and the routed desktop invitation management page.
- Show API-backed publish request, policy check, invitation, and onboarding state in dedicated desktop management pages, with demo fallbacks when the API is unavailable.
- Show provider webhook/update notifications in the desktop `/activity` page so daemon and sync polling inputs are visible.
- Compare two skill refs in the desktop workbench with file patches and semantic manifest changes.
- Inspect locally installed Skill Library managed skills per Claude Code, Cursor, and Codex target from the desktop `/installed` page.
- CLI commands for auth, workspace add/list, local and remote scan, subscribe/unsubscribe, sync, install/list/remove, package, update decisions, status, versions, diff, and rollback reinstall.
- Write CLI lifecycle logs to `~/.skill-library/logs/YYYY-MM-DD.log`; pass `--verbose` to also mirror logs to stderr while keeping stdout available for command output.
- Export local diagnostics with `skill-library diagnostics` or the desktop `/cli` diagnostics controls, including sanitized logs and local state summaries while excluding credentials.
- Route-contract coverage for the desktop management pages, plus local CLI smokes covering scan/subscribe/sync/install/status and offline rollback/pinned lock behavior across Claude Code, Cursor, and Codex target roots.
- Real-provider demo fixture generator, update-push helper, runbook, and executable evidence harness for the final `skill-library-demo-skills` validation pass.
- Docker Compose self-hosting for the API, desktop web preview, and Postgres.

## Remaining MVP Work

- Run and record a full real-provider demo against a prepared `skill-library-demo-skills` repository.

## Development

```bash
rtk pnpm install
rtk cargo test --workspace
rtk pnpm --dir apps/desktop check
rtk pnpm --dir apps/api check
rtk pnpm dev:desktop
rtk pnpm dev:api
```

The API stores MVP management state in Postgres when `DATABASE_URL` is set. Without `DATABASE_URL`, it falls back to `.skill-library-api-state.json`; set `SKILL_LIBRARY_API_STATE_PATH=/path/to/state.json` to choose another JSON state file during local demos or tests.

The CLI can be run from source:

```bash
rtk cargo run -p skill-library-cli -- --help
```
