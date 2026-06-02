# AGENTS.md

This file gives AI coding agents the project map and working rules for Skill Library. Prefer reading the relevant source before making changes; the repo has a mixed React/Tauri/Rust architecture and several generated or cached outputs.

## Project Summary

Skill Library is a Tauri v2 desktop app for discovering, installing, subscribing to, reviewing, and publishing AI skills. The UI is React + TypeScript + HeroUI. The desktop backend and CLI are Rust workspace crates. Local state is stored under `~/.skill-library` and the desktop app talks to Rust through Tauri commands.

## Required Local Command Wrapper

When running local shell commands in this workspace, prefix commands with `rtk`:

```bash
rtk pnpm -r check
rtk cargo check --workspace
rtk pnpm --filter @skill-library/desktop tauri build --debug
```

Use `rg` / `rg --files` for search. Do not revert unrelated dirty files; this repo may contain in-progress user changes.

## Directory Map

```text
.
+-- apps/
|   `-- desktop/                 React + Tauri desktop application
|       +-- src/                 Frontend source
|       |   +-- assets/          Frontend-bundled images and static assets
|       |   +-- context/         React context providers
|       |   +-- hooks/           Frontend hooks, locale/theme/sync helpers
|       |   +-- lib/             Tauri command wrappers, registry/cache helpers
|       |   +-- pages/           Route-level pages
|       |   +-- routes/          TanStack Router route wrappers
|       |   +-- shell/           App shell, sidebar, settings/auth dialogs
|       |   +-- state/           Zustand/global app state
|       |   +-- utils/           Navigation, formatting, risk, visual helpers
|       |   +-- widgets/         Reusable UI widgets and modals
|       |   +-- App.tsx          App root
|       |   +-- main.tsx         React entry point
|       |   +-- router.tsx       Router setup
|       |   `-- styles.css       Theme bridge and app CSS
|       +-- src-tauri/           Tauri Rust app crate
|       |   +-- capabilities/    Tauri permission manifests
|       |   +-- icons/           Native app icons used by bundle config
|       |   +-- src/
|       |   |   +-- lib.rs       Tauri command layer and app setup
|       |   |   +-- db.rs        SQLite schema and persistence helpers
|       |   |   +-- app_icons.rs Local app icon lookup protocol/cache
|       |   |   +-- ai_review.rs AI review helpers
|       |   |   `-- main.rs      Native entry point
|       |   +-- Cargo.toml       Desktop crate dependencies
|       |   `-- tauri.conf.json  Product, window, deep-link, and bundle config
|       +-- package.json         Vite/Tauri frontend scripts
|       `-- vite.config.ts       Vite + React + Tailwind config
+-- crates/
|   +-- skill-library-cli/       Rust CLI entry point
|   +-- skill-library-core/      Shared types, paths, config, credentials
|   +-- skill-library-installer/ Install/remove/link skills into runtime targets
|   +-- skill-library-manifest/  Skill parsing, metadata, risk/semantic changes
|   +-- skill-library-provider/  Provider traits and shared provider models
|   +-- skill-library-provider-github/ GitHub REST/GraphQL/archive implementation
|   +-- skill-library-publish/   Publish package/policy/PR planning logic
|   `-- skill-library-sync/      Workspace/subscription/sync state logic
+-- docs/                        Product, architecture, schema, demo notes
+-- scripts/                     Demo, smoke, and maintenance scripts
+-- .github/workflows/           GitHub Actions workflows
+-- Cargo.toml                   Rust workspace definition
+-- package.json                 Root pnpm scripts
+-- pnpm-workspace.yaml          Frontend workspace packages
`-- pnpm-lock.yaml               pnpm lockfile
```

Generated outputs such as `target/`, `apps/desktop/dist/`, and `node_modules/` are not source. Do not edit them by hand.

## Runtime Architecture

- Frontend calls Rust through wrappers in `apps/desktop/src/lib/skill-library.ts`.
- Tauri commands live in `apps/desktop/src-tauri/src/lib.rs`; keep command inputs/outputs serializable and camelCase for frontend-facing structs.
- SQLite persistence lives in `apps/desktop/src-tauri/src/db.rs`; schema migrations should be idempotent.
- GitHub fetching, publish, comments, pull requests, and archive download behavior belongs in `crates/skill-library-provider-github` or provider-facing logic, not directly in React.
- Install and runtime target behavior belongs in `crates/skill-library-installer` and Tauri command wrappers.
- Skill identity is based on `SKILL.md` metadata. Do not assume a manifest exists; manifests are compatibility inputs only.

## Frontend Guidelines

- Use existing HeroUI v3 patterns and CSS variables from `styles.css`.
- Keep route pages in `pages/`, small route wrappers in `routes/`, and reusable UI in `widgets/`.
- User-facing text should go through `useLocale`; avoid new hard-coded UI copy.
- Heavy editor stacks must stay lazy-loaded. `SkillDetail` and CodeMirror/MDXEditor should not be pulled into the startup chunk.
- Prefer hover-only card actions unless a persistent selected state is part of the workflow.
- For dialogs, use the app's existing modal/AlertDialog patterns and keep close buttons visible.

## Rust Guidelines

- Keep shared domain models in `skill-library-core` or the crate that owns the behavior.
- Keep provider-specific code behind provider traits; do not leak GitHub-only assumptions into generic crates.
- Keep filesystem operations explicit about canonical source paths versus symlink target paths.
- Avoid startup-blocking scans. If a check can be deferred, run it asynchronously after first paint or when the relevant view opens.
- Persist source workspace/path metadata for installed skills when it is needed for updates or subscriptions.

## Local Data Model

The app stores managed data under `~/.skill-library`. Important concepts:

- Canonical installed skills live in the app-managed skills/cache area.
- Runtime targets, such as Codex/Claude-compatible folders, should point to canonical content by symlink or configured copy mode.
- Project-level installations must keep enough SQLite metadata to distinguish the project path, runtime, enabled state, and subscription/update source.
- Deleting a project-level installation should remove the subscription/record only after explicit confirmation; disabling should leave existing data available for future re-enable.

## Common Commands

```bash
rtk pnpm install
rtk pnpm dev
rtk pnpm dev:web
rtk pnpm -r check
rtk pnpm --filter @skill-library/desktop build
rtk cargo fmt --all
rtk cargo check --workspace
rtk cargo test --workspace
rtk cargo run -p skill-library-cli -- --help
rtk pnpm --filter @skill-library/desktop tauri build --debug
```

## Packaging and CI

The desktop build workflow is `.github/workflows/desktop-build.yml`. It builds:

- macOS Intel: `x86_64-apple-darwin` on `macos-15-intel`
- macOS Apple Silicon: `aarch64-apple-darwin` on `macos-15`
- Windows x64: `x86_64-pc-windows-msvc` on `windows-2025`
- Windows ARM64: `aarch64-pc-windows-msvc` on `windows-11-arm`

Tauri bundle artifacts are uploaded by `tauri-apps/tauri-action`. Additional portable artifacts are uploaded separately:

- macOS portable: zipped `.app` bundle
- Windows portable: zipped `skill-library-desktop.exe`

Builds run on pushes to `main`, pull requests targeting `main`, and manual dispatches. Every artifact name includes the resolved app version.

Version source of truth is checked at workflow start. These files must agree before publishing:

- `apps/desktop/package.json`
- `apps/desktop/src-tauri/tauri.conf.json`
- `Cargo.toml` under `[workspace.package]`

After a successful push-to-`main` build, the workflow creates a `v<version>` tag and a draft GitHub Release containing the installer and portable artifacts. If that tag already exists, the release job fails and the version must be bumped first. Pull requests and manual runs build artifacts only; they do not create tags or releases.

The workflow has a cleanup job to control storage cost:

- Keeps the latest three `v*` GitHub Releases and deletes older release records/assets while keeping the git tags.
- Keeps artifacts from the current workflow run and the latest two previous successful desktop build runs.
- Deletes only artifacts with the `skill-library-` prefix.
