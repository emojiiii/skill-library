# Suggested Commands

- Install deps: `rtk pnpm install`.
- Rust all tests: `rtk cargo test --workspace`.
- Rust targeted tests: `rtk cargo test -p skill-library-cli`, `rtk cargo test -p skill-library-sync`.
- Desktop checks/tests: `rtk pnpm --dir apps/desktop check`, `rtk pnpm --dir apps/desktop test`, `rtk pnpm --dir apps/desktop build`.
- API checks/tests: `rtk pnpm --dir apps/api check`, `rtk pnpm --dir apps/api test`, `rtk pnpm --dir apps/api build`.
- Local CLI smoke: `rtk pnpm smoke:cli-keypath`.
- Demo dry run: `rtk pnpm demo:real-provider:dry-run`; final evidence run: `rtk pnpm demo:real-provider` with required GitHub env vars.
- Verify final evidence: `rtk pnpm demo:verify-evidence .skill-library-demo-evidence/<timestamp>`.
- Docker config sanity: `rtk docker compose config`.
- Dev servers: `rtk pnpm dev:desktop`, `rtk pnpm dev:api`.
- Run CLI from source: `rtk cargo run -p skill-library-cli -- --help`.