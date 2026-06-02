# Core

- Workdir: `/Users/cyk/Documents/emojiiii/Skill Library`.
- Always prefix shell commands with `rtk`; repo instructions come from `/Users/cyk/.codex/RTK.md`.
- Product: Skill Library, a Git-provider-backed workflow layer for team AI Skills.
- Top-level layout: Rust workspace in `crates/`; Tauri/React desktop in `apps/desktop`; Hono API in `apps/api`; demo and smoke scripts in `scripts/`; product/technical docs in `docs/`.
- Read `mem:tech_stack` for frameworks/package managers, `mem:conventions` for implementation patterns, `mem:suggested_commands` for common commands, and `mem:task_completion` before finishing changes.
- Current durable completion gate from README/docs: local checks can pass, but full MVP is not complete until real GitHub demo evidence is recorded and verified against a prepared `skill-library-demo-skills` repo.