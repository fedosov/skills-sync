# SkillsSync

Single solution repository: Rust workspace with CLI + Tauri desktop app.

## Workspace

- Root: `/Users/fedosov/Dev/ai-skills-widget/platform`
- Core domain: `skillssync-core`
- CLI: `skillssync-cli`
- Desktop app: `apps/skillssync-desktop` (Tauri + React)

## Run

```bash
cd /Users/fedosov/Dev/ai-skills-widget
./scripts/run-tauri-gui.sh
```

## Test

```bash
cd /Users/fedosov/Dev/ai-skills-widget/platform
cargo test

cd /Users/fedosov/Dev/ai-skills-widget/platform/apps/skillssync-desktop/ui
npm run test:coverage
```

## Details

See `/Users/fedosov/Dev/ai-skills-widget/platform/README.md`.
