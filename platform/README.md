# Dotagents Desktop Platform Workspace

This workspace contains the desktop app only:

- `apps/agent-sync-desktop/src-tauri`: Tauri backend that resolves the bundled runtime, persists app settings, and runs vendor commands.
- `apps/agent-sync-desktop/ui`: React + Vite control plane for skills, MCP servers, and command transcripts.

## Quick start

```bash
cd platform
cargo test
```

## Desktop

```bash
cd platform/apps/agent-sync-desktop/ui
npm install

cd ../src-tauri
cargo tauri dev
```
