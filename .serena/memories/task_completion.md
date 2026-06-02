# Task Completion

- For Rust-only changes: run targeted crate tests first, then `rtk cargo test --workspace` when blast radius crosses crates.
- For desktop changes: run `rtk pnpm --dir apps/desktop check` and `rtk pnpm --dir apps/desktop test`; use browser smoke for significant UI changes.
- For API changes: run `rtk pnpm --dir apps/api check` and `rtk pnpm --dir apps/api test`.
- For CLI key-path/install/sync behavior: run `rtk pnpm smoke:cli-keypath`.
- For demo scripts/evidence behavior: run relevant `rtk bash -n scripts/...`, `rtk pnpm demo:real-provider:dry-run`, and evidence verifier when a real evidence folder exists.
- Full MVP completion requires real-provider GitHub demo execution plus required screenshots and `rtk pnpm demo:verify-evidence .skill-library-demo-evidence/<timestamp>` passing.