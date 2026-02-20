# SkillsSync Platform Workspace

This workspace contains the multiplatform refactor:

- `crates/skillssync-core`: shared domain engine and file-sync use-cases
- `crates/skillssync-cli`: `skillssync` CLI on top of `skillssync-core`
- `apps/skillssync-desktop/src-tauri`: Tauri shell exposing core commands
- `apps/skillssync-desktop/ui`: React + Vite frontend for desktop app
- `spec/`: `state.json` schema, fixtures, CLI contract, and platform capability matrix

## Quick start

```bash
cd /Users/fedosov/Dev/ai-skills-widget/platform
cargo test
cargo run -p skillssync-cli -- sync --trigger manual --json
```

## Desktop

```bash
cd /Users/fedosov/Dev/ai-skills-widget/platform/apps/skillssync-desktop/ui
npm install

cd /Users/fedosov/Dev/ai-skills-widget/platform/apps/skillssync-desktop/src-tauri
cargo tauri dev
```
