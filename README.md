# SkillsSync

Keep one canonical catalog for `skills`, `subagents`, and managed `MCP servers`, then sync it across agent runtimes (`Claude Code`, `Cursor`, `Codex`, and others).

If an item exists in one ecosystem but is missing in another, SkillsSync reconciles it by rebuilding managed links and updating managed registry entries.

## Screenshot

![SkillsSync screenshot](docs/images/skillssync-latest-screenshot.png?v=20260221-121733)

## What SkillsSync Solves

- Stops drift across `skills` and `subagents` roots:
  - `skills`: `~/.claude/skills`, `~/.agents/skills`, `~/.codex/skills`
  - `subagents`: `~/.claude/agents`, `~/.cursor/agents`, `~/.agents/subagents`
- Prevents "it exists in Claude/Cursor but does not appear in Codex" situations.
- Gives a safe lifecycle for `skills`: archive, restore, promote project skills to global, rename, delete.
- Provides transparent `subagent` sync diagnostics in desktop UI (canonical source, targets, link status).
- Centralizes managed MCP servers and propagates them to Codex, Claude local settings, and project `.mcp.json`.
- Keeps sync behavior deterministic with explicit conflict handling.

## Why This Exists

Without synchronization, teams accumulate duplicate and stale skills/subagents across multiple agent directories. That creates inconsistent behavior between tools and broken expectations for users.

SkillsSync provides one sync engine that discovers skills/subagents/MCP servers, validates consistency, and applies a managed cross-agent layout.

## How Sync + Validation Works

Validation is part of the normal sync cycle (not a separate tool):

1. Discover `skill` packages and `subagent` markdown configs in global and project roots.
2. Compare duplicates by `skill_key` and content hash.
3. Mark conflicts when same key has different content (for both object types).
4. Optionally migrate canonical sources to Claude roots via `auto_migrate_to_canonical_source`.
5. Rebuild/update managed symlinks for target agent directories.
6. Update managed blocks in `~/.codex/config.toml`:
   - skills: `# skills-sync:begin` ... `# skills-sync:end`
   - subagents: `# skills-sync:subagents:begin` ... `# skills-sync:subagents:end`
7. Reconcile managed MCP catalog:
   - central source: `~/.config/ai-agents/config.toml` (`# skills-sync:mcp:begin` ... `# skills-sync:mcp:end`)
   - codex target: `~/.codex/config.toml` (`# skills-sync:mcp:codex:begin` ... `# skills-sync:mcp:codex:end`)
   - claude global target: prefer `~/.claude.json` (`mcpServers`), fallback `~/.claude/settings.local.json`
   - claude project targets: workspace `.mcp.json` (when canonical) or `~/.claude.json` (`projects.<workspace>.mcpServers`)
   - project codex target: existing workspace `.codex/config.toml`

Result: once sync succeeds, cross-agent visibility is reconciled automatically.

## Cleanup Workflow (UI-first, safe)

Use the desktop app to review and confirm each mutation explicitly.

Supported actions:

- `Archive`: move active skill into runtime archives.
- `Restore`: bring archived skill back as active global skill.
- `Make global`: promote active project skill to global scope.
- `Rename`: normalize skill key from the new title and move path safely.
- `Delete`: remove active skill (moves payload to trash) or remove archived bundle.

All destructive/structural actions require confirmation.

`Subagents` in v1.1 are `sync + inspect` (read-only lifecycle): discover, validate, sync, and inspect source/targets/symlink status.

## Quickstart (Desktop)

```bash
git clone https://github.com/fedosov/skills-sync.git
cd skills-sync
./scripts/run-tauri-gui.sh
```

## Headless Linux (CLI)

### One-shot sync

```bash
cd platform
cargo run -p skillssync-cli -- sync --trigger manual --json
```

### List subagents

```bash
cd platform
cargo run -p skillssync-cli -- list-subagents --scope all --json
```

### MCP management

```bash
cd platform
cargo run -p skillssync-cli -- mcp list --json
cargo run -p skillssync-cli -- mcp set-enabled --server exa --agent codex --enabled false
cargo run -p skillssync-cli -- mcp set-enabled --server exa --agent project --enabled false --scope project --workspace /Users/example/Dev/workspace-a
cargo run -p skillssync-cli -- mcp sync
```

### Optional environment diagnostics

```bash
cd platform
cargo run -p skillssync-cli -- doctor
```

### systemd service + timer (copy-paste example)

Build the binary first:

```bash
cd /opt/skills-sync/platform
cargo build -p skillssync-cli --release
sudo install -m 0755 target/release/skillssync /usr/local/bin/skillssync
```

Create `/etc/systemd/system/skillssync-sync.service`:

```ini
[Unit]
Description=SkillsSync manual sync run
After=network.target

[Service]
Type=oneshot
User=%i
ExecStart=/usr/local/bin/skillssync sync --trigger manual --json
```

Create `/etc/systemd/system/skillssync-sync.timer`:

```ini
[Unit]
Description=Run SkillsSync sync every 15 minutes

[Timer]
OnBootSec=2min
OnUnitActiveSec=15min
Unit=skillssync-sync.service

[Install]
WantedBy=timers.target
```

Enable timer:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now skillssync-sync.timer
systemctl list-timers | rg skillssync
```

If you need event-driven continuous mode instead of interval mode, use:

```bash
skillssync watch
```

## Prerequisites

- Rust and Cargo
- Node.js and npm
- Tauri system dependencies installed for your OS

## Run Tests

```bash
cd platform
cargo test
```

```bash
cd platform/apps/skillssync-desktop/ui
npm run test:coverage
```

## Repository Layout

- `platform/crates/skillssync-core`: shared sync engine
- `platform/crates/skillssync-cli`: command-line interface
- `platform/apps/skillssync-desktop`: desktop app (Tauri + React)

## Contributing

See `CONTRIBUTING.md`.
