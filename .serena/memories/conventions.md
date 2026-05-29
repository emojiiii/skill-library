# Conventions

- Use `apply_patch` for manual file edits.
- Rust CLI prints machine-readable command results to stdout; prompts/errors/logs should use stderr or files so scripts can parse stdout JSON.
- App state lives under `~/.team-ai-hub` via `AppPaths`; tests/smokes should use isolated HOME or explicit target roots to avoid touching real Claude Code/Cursor/Codex state.
- GitHub tokens must be read from args/env/keychain but not echoed into logs or evidence; demo scripts redact token-like values.
- Risky install/sync/rollback/publish flows require explicit confirmation unless `--yes` is supplied.
- API errors use structured envelope `{ ok:false, error:{ code,message,details? } }`; CLI `friendly_api_error` keeps structured and legacy API errors readable.
- Desktop has API-unavailable demo fallbacks in `apps/desktop/src/lib/teamai.ts`; keep real-provider paths separate from fallback data.
- Keep UI dense/workflow-focused; avoid marketing landing-page treatment for the operational desktop workbench.