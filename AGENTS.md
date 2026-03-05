# Agent Instructions

Agent Sync is a strict dotagents sync platform with a Tauri desktop app and Rust CLI.

## Quick Start

Three commands to validate any change:

```sh
make test        # Rust tests (includes architecture guards)
make lint        # Rust clippy + fmt + UI eslint/oxlint/prettier
./scripts/run-tauri-gui.sh   # Launch desktop app locally
```

For UI-only changes: `cd platform/apps/agent-sync-desktop/ui && npm run test`

## Scope and Precedence

- This file is the repository-local operating contract for AI agents in this project.
- If global/default agent rules conflict with this file, follow this file for this repository.
- `CLAUDE.md` is a compatibility shim that points to this file.

## Repository Map

- `platform/` — Rust workspace root.
- `platform/crates/agent-sync-core/` — core sync engine, models, paths, registries.
- `platform/crates/agent-sync-cli/` — strict CLI (`agent-sync`) over `agent-sync-core`.
- `platform/apps/agent-sync-desktop/src-tauri/` — Tauri backend/commands.
- `platform/apps/agent-sync-desktop/ui/` — React + TypeScript + Vite frontend.
- `platform/spec/` — API contracts and schema (`cli-contract.json`, `state.schema.json`, `capability-matrix.json`).
- `scripts/` — development helpers (`run-tauri-gui.sh`, hook installer).

## Deep Documentation

- Architecture, layers, dependency rules: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- Design decisions catalog: [`docs/design-docs/index.md`](docs/design-docs/index.md)
- Setup and operations: [`docs/SETUP.md`](docs/SETUP.md)
- macOS signing: [`docs/macos-signing.md`](docs/macos-signing.md)
- dotagents migration: [`docs/dotagents-migration.md`](docs/dotagents-migration.md)

## Source of Truth

- Prefer executable sources over prose docs when they conflict:
  - Commands: `Makefile`, UI `package.json`, CLI `src/main.rs`.
  - Contracts: `platform/spec/*.json`.
  - Runtime behavior: `agent-sync-core` and Tauri command handlers.
- Treat generated/runtime folders as non-authoritative for design decisions (`platform/target/`, caches, bundle outputs).

## Environment Requirements

- Rust stable toolchain.
- Node.js 22+ and npm.
- Tauri CLI (`cargo install tauri-cli`) for desktop run/build.
- `dotagents` on PATH is required for standalone strict CLI migration/setup flows.
- Desktop app bundles dotagents runtime for normal app usage.

## Verified Commands

Prefer Makefile targets over raw commands.

| Task | Command |
|---|---|
| Full lint | `make lint` |
| Fix lint | `make lint-fix` |
| Workflow lint | `make lint-workflows` |
| Rust lint only | `make lint-rust` |
| UI lint only | `make lint-ui` |
| Rust tests | `make test` |
| TS typecheck | `make typecheck-ts` |
| Rust check | `make check-rust` |
| Run desktop app | `./scripts/run-tauri-gui.sh` |
| UI tests | `cd platform/apps/agent-sync-desktop/ui && npm run test` |
| UI test coverage | `cd platform/apps/agent-sync-desktop/ui && npm run test:coverage` |
| UI e2e tests | `make test-e2e` |
| Arch guard | `make check-arch` |

## Strict CLI Surface

Current strict CLI supports:

- `agent-sync sync --scope <all|user|project> [--json]`
- `agent-sync watch --scope <all|user|project> [--interval-seconds N]`
- `agent-sync skills <install|list|add|remove|update> ...`
- `agent-sync mcp <list|add|remove|fix-unmanaged-claude> ...`
- `agent-sync migrate-dotagents --scope <all|user|project>`
- `agent-sync doctor`

Legacy lifecycle commands are removed in strict mode. Check `platform/spec/cli-contract.json` before documenting or adding CLI behavior.

## Working Rules

- No mandatory post-iteration build/copy step.
- When borrowing ideas, patterns, or configs from external repositories, append source notes to `INSPIRATIONS.md`.
- Keep changes focused; do not edit unrelated files.
- Add or update tests for behavior changes.
- Run validation appropriate to your change scope before finalizing (`make lint`, `make test`, targeted UI tests).
- See `CONTRIBUTING.md` for commit and PR expectations.

## Area-Specific Guidance

### Rust Core (`platform/crates/agent-sync-core`)

- Preserve sync determinism and conflict reporting semantics.
- Keep schema/state compatibility in mind when changing models/serialization.
- Avoid hidden filesystem side effects; respect existing write-control patterns.

### CLI (`platform/crates/agent-sync-cli`)

- Keep flags and output aligned with `platform/spec/cli-contract.json`.
- If command behavior changes, update contract docs and tests in same change.

### Tauri Backend (`platform/apps/agent-sync-desktop/src-tauri`)

- Keep command signatures and payloads backward-compatible with UI types unless intentionally versioned.
- Respect runtime controls for filesystem writes and sync triggers.

### UI (`platform/apps/agent-sync-desktop/ui`)

- Maintain strict TypeScript quality: avoid `any`/`unknown` where concrete typing is possible.
- Keep `tauriApi.ts` and `types.ts` synchronized with backend command payloads.
- Add or update Vitest/RTL tests for user-visible behavior changes.

## Contract and Compatibility Checklist

Before merging changes that affect interfaces or behavior, verify impact in:

- `platform/spec/cli-contract.json`
- `platform/spec/state.schema.json`
- `platform/spec/capability-matrix.json`
- `platform/apps/agent-sync-desktop/ui/src/types.ts`
- `platform/apps/agent-sync-desktop/ui/src/tauriApi.ts`

If behavior changes, update contracts/docs/tests in the same PR to avoid drift.
