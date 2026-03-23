# Architecture

Dotagents Desktop is a Tauri v2 desktop app that wraps `@sentry/dotagents` via `npx`.
It does NOT own a sync engine or CLI — it delegates all agent operations to the pinned vendor binary.

## Domain Map

```
┌─────────────────────────────────────────────┐
│                   UI (React)                │
│  App.tsx → tauriApi.ts → Tauri invoke       │
├─────────────────────────────────────────────┤
│              Tauri Backend (Rust)            │
│  main.rs → commands → dotagents_runner      │
│                    ↓                        │
│            dotagents_runtime (npx)          │
│            settings / app_state             │
├─────────────────────────────────────────────┤
│           Vendor: @sentry/dotagents         │
│           (pinned v1.4.0 via npx)           │
└─────────────────────────────────────────────┘
```

## Layers (top → bottom)

| Layer | Location | Responsibility |
|-------|----------|----------------|
| UI | `ui/src/` | React components, Tauri API bindings, user interaction |
| Commands | `src-tauri/src/main.rs` | Tauri command handlers, request validation |
| Runner | `src-tauri/src/dotagents_runner.rs` | Process execution, stdout/stderr capture, transcript |
| Runtime | `src-tauri/src/dotagents_runtime.rs` | npx resolution, version pinning |
| Settings | `src-tauri/src/settings.rs` | Persistent config, scope model |
| State | `src-tauri/src/app_state.rs` | Shared application state |

## Dependency Rules

- **Direction**: UI → Commands → Runner → Runtime. Never reverse.
- **UI ↔ Backend contract**: `types.ts` and `tauriApi.ts` must stay synchronized with Rust command signatures.
- **No vendor bypass**: All dotagents operations go through `dotagents_runner`, never direct process spawning from commands.
- **Runtime isolation**: Only `dotagents_runtime.rs` knows about npx resolution.

## Scope Model

Two scopes, no more:
- **project** — requires a selected project folder, commands run in that directory
- **user** — commands run with `--user` flag, no project folder needed

## Architectural Guards

Enforced by `scripts/check-architecture.sh` (run via `make check-arch`):
- Workspace must include the desktop Tauri app
- Deleted crates (`agent-sync-core`, `agent-sync-cli`, `spec`) must stay removed
- Runtime must use npx-pinned approach
- Product metadata must be "Dotagents Desktop"

## Key Interfaces

- **Backend → Frontend**: Tauri `invoke()` commands defined in `main.rs`
- **Frontend → Backend**: `tauriApi.ts` wraps all `invoke()` calls with typed signatures
- **Shared types**: `types.ts` mirrors Rust response structs
