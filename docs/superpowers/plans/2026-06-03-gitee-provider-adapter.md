# Gitee Provider Adapter Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the Phase 4 Gitee adapter read path so public Gitee and token-authenticated Gitee repositories can be used as provider-backed skill sources.

**Architecture:** Create `skill-library-provider-gitee` implementing `SkillSourceProvider`, `GitRepositoryProvider`, and `ArchiveProvider` against Gitee OpenAPI v5. Reuse the generic remote scan fallback added for GitLab. Keep enterprise/private Gitee as provider-instance configuration only until real API differences are verified.

**Tech Stack:** Rust workspace crate, `reqwest`, `async-trait`, `serde`, `base64`, `flate2`/`tar`, `mockito`, existing provider traits.

---

## Scope

- Gitee user repository listing.
- Single repo metadata lookup.
- Recursive tree listing through git tree API.
- File content reads through contents API.
- Tags, releases, compare, collaborator permission.
- Tarball download and extraction.
- Provider factory and default `gitee.com` enablement.

Out of scope:

- Enterprise/private Gitee API variants beyond configurable `ProviderInstance`.
- Pull Request publishing.
- Gitee webhook/member/invitation UI.
