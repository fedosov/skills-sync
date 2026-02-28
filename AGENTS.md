# Agent Instructions

Agent Sync Desktop — Tauri desktop app (Rust backend + React/TypeScript UI).

## Project Structure

- `platform/` — Cargo workspace root
- `platform/crates/` — shared Rust crates (core engine)
- `platform/apps/agent-sync-desktop/src-tauri/` — Tauri app (Rust side)
- `platform/apps/agent-sync-desktop/ui/` — React/Vite frontend
- `scripts/` — helper scripts

## Commands

| Task | Command |
|---|---|
| Full lint | `make lint` |
| Fix lint | `make lint-fix` |
| Rust tests | `make test` or `cd platform && cargo test --workspace` |
| UI tests | `cd platform/apps/agent-sync-desktop/ui && npm run test` |
| Run app | `./scripts/run-tauri-gui.sh` |

## Rules

- No mandatory post-iteration build/copy step.
- When borrowing ideas, patterns, or configs from external repositories — record the source in `INSPIRATIONS.md`.
