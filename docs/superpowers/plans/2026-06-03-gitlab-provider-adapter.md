# GitLab Provider Adapter Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the Phase 3 GitLab adapter read path so GitLab.com and self-hosted GitLab instances can be used as provider-backed skill sources.

**Architecture:** Create a new `skill-library-provider-gitlab` crate that implements `SkillSourceProvider`, `GitRepositoryProvider`, and `ArchiveProvider` using GitLab REST API v4. Wire it into the existing provider factory without changing GitHub behavior. Leave Merge Request publishing and UI polish for later phases.

**Tech Stack:** Rust workspace crate, `reqwest`, `async-trait`, `serde`, `flate2`/`tar`, `mockito`, existing `skill-library-provider` traits.

---

## Scope

This plan implements the minimum Phase 3 read/archive path:

- GitLab project list and single project lookup.
- Repository tree listing and raw file reads.
- Tags, releases, compare refs, and permission mapping.
- Repository archive download with progress and extraction.
- `ProviderFactory` builds GitLab providers for `gitlab.com` and custom GitLab instances.

Out of scope:

- Merge Request creation.
- GitLab invitations, webhooks, issues/comments, and social features.
- UI provider picker polish beyond existing generic commands.
- Gitee and WebDAV adapters.

## Task 1: Add GitLab Provider Crate

**Files:**
- Create: `crates/skill-library-provider-gitlab/Cargo.toml`
- Create: `crates/skill-library-provider-gitlab/src/lib.rs`
- Modify: `Cargo.toml`

- [ ] Add the crate to the workspace.
- [ ] Implement `GitLabProvider::for_instance`, `GitLabProvider::anonymous`, and `GitLabProvider::with_instance_base_url`.
- [ ] Map GitLab HTTP errors into `ProviderError`.
- [ ] Add URL encoding helpers for project paths, file paths, refs, and query params.

## Task 2: Implement Read Traits

**Files:**
- Modify: `crates/skill-library-provider-gitlab/src/lib.rs`

- [ ] Implement `SkillSourceProvider::list_sources` using GitLab projects.
- [ ] Implement `SkillSourceProvider::get_source` for `owner/repo` and nested namespaces.
- [ ] Implement recursive `list_files` from the repository tree endpoint.
- [ ] Implement `read_file` from the raw repository file endpoint.
- [ ] Implement `download_snapshot` from GitLab repository archive.

## Task 3: Implement Git Repository Traits

**Files:**
- Modify: `crates/skill-library-provider-gitlab/src/lib.rs`

- [ ] Implement tags and releases listing.
- [ ] Implement ref comparison.
- [ ] Implement permission mapping from GitLab access levels.
- [ ] Return unsupported/empty behavior for capabilities that are not part of this phase.

## Task 4: Add Contract Tests

**Files:**
- Modify: `crates/skill-library-provider-gitlab/src/lib.rs`

- [ ] Test nested namespace project lookup is URL encoded.
- [ ] Test recursive tree entries map to provider `FileEntry`.
- [ ] Test raw file reads decode bytes without assuming UTF-8.
- [ ] Test archive extraction returns the real extracted root.
- [ ] Test 401/403/404 map to provider errors consistently.

## Task 5: Wire Factory And Verify

**Files:**
- Modify: `crates/skill-library-sync/Cargo.toml`
- Modify: `crates/skill-library-sync/src/provider_factory.rs`

- [ ] Add `skill-library-provider-gitlab` as a sync dependency.
- [ ] Add `ProviderKind::GitLab` build support.
- [ ] Keep unsupported providers returning `SyncError::ProviderUnsupported`.
- [ ] Run `rtk cargo fmt --all`.
- [ ] Run `rtk cargo test -p skill-library-provider-gitlab`.
- [ ] Run `rtk cargo check --workspace`.
- [ ] Run `rtk cargo test --workspace`.
- [ ] Run `rtk pnpm -r check`.
