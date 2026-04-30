# Agent Instructions

Dotagents Desktop is a desktop-only Tauri wrapper around pinned `@sentry/dotagents` 1.4.0 via `npx`.

## Quick Start

```sh
make lint
make test
cd platform/apps/agent-sync-desktop/ui && npm run test
./scripts/run-tauri-gui.sh
```

## Scope and Precedence

- This file is the repository-local operating contract for AI agents.
- If global/default agent rules conflict with this file, follow this file.
- `CLAUDE.md` is a compatibility shim that points here.

## Repository Map

- `platform/` — Rust workspace root for the desktop app.
- `platform/apps/agent-sync-desktop/src-tauri/` — Tauri backend (runtime, settings, commands).
- `platform/apps/agent-sync-desktop/ui/` — React + TypeScript + Vite frontend.
- `docs/` — architecture, product specs, design docs.
- `scripts/` — development helpers (`run-tauri-gui.sh`, `check-architecture.sh`).

## Deep Dives

| Topic | Location |
|-------|----------|
| Architecture & layers | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| Product specs & anti-patterns | [`docs/product-specs/index.md`](docs/product-specs/index.md) |
| Design decisions | [`docs/design-docs/index.md`](docs/design-docs/index.md) |

## Source of Truth

- Executable sources over prose: `Makefile`, `package.json`, `main.rs`.
- Runtime behavior: `platform/apps/agent-sync-desktop/src-tauri/src/`.
- UI contract: `ui/src/types.ts` and `ui/src/tauriApi.ts`.
- Ignore generated folders: `platform/target/`, `ui/dist/`, coverage outputs.

## Verified Commands

Prefer Makefile targets over raw commands.

| Task | Command |
|------|---------|
| Full lint | `make lint` |
| Fix lint | `make lint-fix` |
| Rust tests | `make test` |
| UI tests | `cd platform/apps/agent-sync-desktop/ui && npm run test` |
| UI e2e | `make test-e2e` |
| TS typecheck | `make typecheck-ts` |
| Arch guard | `make check-arch` |
| Run app | `./scripts/run-tauri-gui.sh` |

## Working Rules

- Use Conventional Commits (`fix(ui): preserve output transcript`).
- Keep changes focused; do not edit unrelated files.
- Add or update tests for behavior changes.
- Run validation appropriate to change scope before finalizing.
- When borrowing from external repos, note in `INSPIRATIONS.md`.

## Area-Specific Guidance

### Tauri Backend (`src-tauri/`)

- Keep command surface aligned with product contract (see `docs/product-specs/`).
- All dotagents operations go through `dotagents_runner.rs`, never direct process spawn.
- Keep runtime pinned to declared vendor version.
- Results must be transcript-friendly: command, cwd, scope, exit code, duration, stdout, stderr.

### UI (`ui/`)

- Strict TypeScript: no `any`, concrete types where possible.
- Keep `tauriApi.ts` and `types.ts` synchronized with backend payloads.
- UI focus: Skills, MCP, Output. See anti-patterns in `docs/product-specs/`.
- Add Vitest/RTL tests for user-visible behavior changes.

## Compatibility Checklist

Before merging interface/behavior changes, verify impact in:

- `src-tauri/src/main.rs`, `dotagents_runner.rs`, `dotagents_runtime.rs`
- `ui/src/types.ts`, `ui/src/tauriApi.ts`

## Environment Requirements

- Rust stable, Node.js 22+, npm
- Tauri CLI (`cargo install tauri-cli`)
- Tauri OS prerequisites: [tauri.app](https://v2.tauri.app/start/prerequisites/)

## Agent skills

### Issue tracker

Issues live in GitHub Issues at `fedosov/agent-sync` (use `gh` CLI). See `docs/agents/issue-tracker.md`.

### Triage labels

Default canonical vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout: `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.
