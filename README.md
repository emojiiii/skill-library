# Skill Library

[中文](./README.zh-CN.md)

**Skill Library is a local-first desktop app and CLI for discovering, reviewing, installing, syncing, and publishing AI Skills across teams.**

## What Is Skill Library?

Skill Library is a Git-provider-backed workflow layer for team AI Skills. Keep your Skills in repositories, then use Skill Library to browse them, review them, subscribe to them, install them into local agent runtimes, roll them back, and publish improvements through normal Git collaboration.

It does not replace GitHub, GitLab, Gitee, WebDAV, pull requests, branch protection, or audit history. It uses those systems as the source of truth and adds the missing product layer for AI workflow assets.

## Problems It Solves

- **Skills are hard to distribute consistently** across teammates and machines.
- **Non-engineers struggle with raw repositories**, branches, tags, and manifest files.
- **Agent runtimes use different folders**, so the same Skill gets copied and drifted.
- **Installing unknown Skills is risky** without diffs, policy checks, and review signals.
- **Local improvements need a team workflow**, not copy-paste into a shared folder.

## Key Features

- **Workspace management**: Treat a Git repository as a team or personal Skill workspace.
- **Skill discovery**: Scan `SKILL.md`, frontmatter, and compatible manifests.
- **Provider architecture**: GitHub support plus provider crates for GitLab, Gitee, and WebDAV.
- **Cross-agent installation**: Install one canonical Skill into Claude Code, Cursor, Codex, and other runtimes.
- **Subscriptions and sync**: Subscribe to remote Skills, pull updates, and maintain local lock state.
- **Diff and rollback**: Compare refs/tags, inspect changes, and restore previous versions.
- **Risk confirmation**: Require explicit confirmation for medium-or-higher risk operations.
- **AI review**: Review local or remote Skills for security and quality before adoption.
- **Publish PR flow**: Package a local Skill and open a reviewable pull request into a team workspace.
- **Collaboration surfaces**: Manage publish PRs, comments, invitations, activity, and notifications.
- **Diagnostics export**: Export sanitized logs and local state for troubleshooting.
- **Local-first operation**: Installed Skills live under `~/.skill-library` and remain usable offline.

## Who It Is For

- **Individual developers** who want their Skills available across machines and agents.
- **Team members** who need a safe way to install approved team workflows.
- **Skill authors** who want to publish local improvements through reviewable PRs.
- **Team admins** who want Git-native governance for AI assets.

## Quick Start

```bash
pnpm install
pnpm dev
```

Run the desktop web preview:

```bash
pnpm dev:web
```

Run the CLI:

```bash
cargo run -p skill-library-cli -- --help
```

Run checks:

```bash
pnpm -r check
cargo check --workspace
cargo test --workspace
```

## Repository Layout

```text
apps/desktop/                 Tauri v2 + React desktop app
  src/                        Frontend source
  src-tauri/                  Rust command layer and desktop backend
crates/skill-library-cli/     Rust CLI
crates/skill-library-core/    Shared models, paths, config, credentials
crates/skill-library-installer/
                              Runtime install/remove/link logic
crates/skill-library-manifest/
                              Skill parsing, metadata, risk and semantic changes
crates/skill-library-provider/
                              Provider traits and shared provider models
crates/skill-library-provider-github/
                              GitHub implementation
crates/skill-library-provider-gitlab/
                              GitLab implementation
crates/skill-library-provider-gitee/
                              Gitee implementation
crates/skill-library-provider-webdav/
                              WebDAV implementation
crates/skill-library-publish/ Publish package and policy logic
crates/skill-library-sync/    Subscription, sync, diff and rollback logic
docs/                         Product, architecture, schema and demo notes
scripts/                      Demo, smoke and maintenance scripts
```

## Local Data

By default, Skill Library stores managed data in:

```text
~/.skill-library/
  db.sqlite
  skills/
  logs/
```

Canonical Skill files live in `~/.skill-library/skills/`. Agent runtime folders can link to that content through symlink mode or receive copied content through copy mode.

## License

MIT
