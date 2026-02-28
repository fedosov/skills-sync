# Agent Sync Desktop

Tauri desktop app (Rust backend + React/TypeScript UI).

## Project Structure

- `platform/` — Cargo workspace root
- `platform/crates/` — shared Rust crates (core engine)
- `platform/apps/agent-sync-desktop/src-tauri/` — Tauri app (Rust side)
- `platform/apps/agent-sync-desktop/ui/` — React/Vite frontend
- `scripts/` — helper scripts

## Commands

Prefer Makefile targets over direct commands.

| Task | Command |
|---|---|
| Full lint | `make lint` |
| Fix lint | `make lint-fix` |
| Rust tests | `make test` or `cd platform && cargo test --workspace` |
| UI tests | `cd platform/apps/agent-sync-desktop/ui && npm run test` |
| TS typecheck | `make typecheck-ts` |
| Rust check | `make check-rust` |
| Run app | `./scripts/run-tauri-gui.sh` |

## Code Quality

- No `any` or `unknown` types in TypeScript.
- Do not add comments or jsdoc to code you did not change.
- Run `make lint-fix` after a series of edits.
- Run `make test` after significant changes.

## Rules

- No mandatory post-iteration build/copy step.
- When borrowing ideas, patterns, or configs from external repositories — record the source in `INSPIRATIONS.md`.
- See `CONTRIBUTING.md` for commit message style and PR guidelines.
