# Agent Sync Platform Workspace

This workspace contains the multiplatform sync engine for `skills` and `subagents`:

- `crates/agent-sync-core`: shared domain engine and file-sync use-cases
- `crates/agent-sync-cli`: `agent-sync` CLI on top of `agent-sync-core`
- `apps/agent-sync-desktop/src-tauri`: Tauri shell exposing core commands
- `apps/agent-sync-desktop/ui`: React + Vite frontend for desktop app
- `spec/`: `state.json` schema, fixtures, CLI contract, and platform capability matrix

## Quick start

```bash
cd platform
cargo test
cargo run -p agent-sync-cli -- sync --scope all --json
cargo run -p agent-sync-cli -- skills list --scope all --json
```

## Desktop

```bash
cd platform/apps/agent-sync-desktop/ui
npm install

cd ../src-tauri
cargo tauri dev
```
