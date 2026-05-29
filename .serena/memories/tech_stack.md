# Tech Stack

- Rust workspace, edition 2021, version `0.1.0`; crates: `teamai-core`, `teamai-manifest`, `teamai-provider`, `teamai-provider-github`, `teamai-installer`, `teamai-sync`, `teamai-publish`, `teamai-cli`.
- Rust key deps: `anyhow`, `thiserror`, `serde`, `tokio`, `reqwest` rustls, `clap`, `tracing`, `tracing-subscriber`, `keyring`, `semver`, `similar`, `tar`, `walkdir`.
- JS package manager: `pnpm@9.15.4`; monorepo scripts in root `package.json`.
- Desktop: Tauri v2 + React 19 + Vite 6 + TypeScript + TanStack React Query/Router + HeroUI v3 + lucide-react.
- API: Hono on Node with TypeScript, pino, zod, optional Postgres via `pg`; without `DATABASE_URL` falls back to local JSON state.
- Docker Compose runs API, desktop web preview, and Postgres.