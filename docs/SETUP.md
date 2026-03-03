# Agent Sync Setup and Operations

This guide contains the full setup and operational details that were removed from the minimal root `README.md`.

## Prerequisites

- Rust stable toolchain and Cargo
- Node.js 22+ and npm
- Tauri CLI:
  - `cargo install tauri-cli`
- Tauri system dependencies for your OS:
  - [https://v2.tauri.app/start/prerequisites/](https://v2.tauri.app/start/prerequisites/)
- For standalone CLI strict flows: `dotagents` on `PATH`
  - `npm install -g @sentry/dotagents@0.10.0`

Desktop bundles include a self-contained `dotagents` runtime and do not require a global `dotagents` install.

## GUI Startup by OS

### macOS

Quick start:

```bash
./scripts/run-tauri-gui.sh
```

Manual start:

```bash
cd platform/apps/agent-sync-desktop/ui
npm install
cd ../src-tauri
cargo tauri dev
```

### Windows (PowerShell)

Quick sequence:

```powershell
cd platform/apps/agent-sync-desktop/ui
npm install
cd ../src-tauri
cargo tauri dev
```

One-liner:

```powershell
cd platform/apps/agent-sync-desktop/ui; npm install; cd ../src-tauri; cargo tauri dev
```

### Linux

Quick start:

```bash
./scripts/run-tauri-gui.sh
```

Manual start:

```bash
cd platform/apps/agent-sync-desktop/ui
npm install
cd ../src-tauri
cargo tauri dev
```

## CLI Flows

All commands below can be run through Cargo from the repository root.

### Sync (strict one-shot)

```bash
cd platform
cargo run -p agent-sync-cli -- sync --scope all --json
```

### Contract migration (required before strict sync in uninitialized environments)

```bash
cd platform
cargo run -p agent-sync-cli -- migrate-dotagents --scope all
```

For step-by-step migration and rollback, see [dotagents-migration.md](dotagents-migration.md).

### Skills management

```bash
cd platform
cargo run -p agent-sync-cli -- skills install --scope all
cargo run -p agent-sync-cli -- skills list --scope project --json
cargo run -p agent-sync-cli -- skills add owner/repo --scope project
cargo run -p agent-sync-cli -- skills remove owner/repo --scope project
cargo run -p agent-sync-cli -- skills update --scope all
```

### MCP management

```bash
cd platform
cargo run -p agent-sync-cli -- mcp list --scope all --json
cargo run -p agent-sync-cli -- mcp add exa --scope project
cargo run -p agent-sync-cli -- mcp remove exa --scope project
```

### Environment diagnostics

```bash
cd platform
cargo run -p agent-sync-cli -- doctor
```

### Continuous watch mode

```bash
cd platform
cargo run -p agent-sync-cli -- watch --scope all --interval-seconds 15
```

## Linux systemd service + timer example

Build and install the binary first:

```bash
cd /opt/agent-sync/platform
cargo build -p agent-sync-cli --release
sudo install -m 0755 target/release/agent-sync /usr/local/bin/agent-sync
```

Create `/etc/systemd/system/agent-sync-sync.service`:

```ini
[Unit]
Description=Agent Sync manual sync run
After=network.target

[Service]
Type=oneshot
User=%i
ExecStart=/usr/local/bin/agent-sync sync --scope all --json
```

Create `/etc/systemd/system/agent-sync-sync.timer`:

```ini
[Unit]
Description=Run Agent Sync sync every 15 minutes

[Timer]
OnBootSec=2min
OnUnitActiveSec=15min
Unit=agent-sync-sync.service

[Install]
WantedBy=timers.target
```

Enable timer:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now agent-sync-sync.timer
systemctl list-timers | rg agent-sync
```

## Lint and Tests

From repository root:

```bash
make lint
make lint-fix
make test
```

UI tests directly:

```bash
cd platform/apps/agent-sync-desktop/ui
npm run test
npm run test:coverage
```

## Sync and Validation Behavior

Validation is part of each sync cycle:

1. Discover `skill` packages and `subagent` markdown configs in global and project roots.
2. Build project workspace candidates from stable checkout roots (`~/Dev` and configured discovery roots), then ignore any path inside `~/.codex/worktrees`.
3. Compare duplicates by `skill_key` and content hash.
4. Mark conflicts when the same key has different content.
5. Optionally migrate canonical sources via `auto_migrate_to_canonical_source`.
6. Rebuild managed symlinks for target agent directories.
7. Update managed blocks in `~/.codex/config.toml`.
8. Reconcile managed MCP catalog across shared and runtime-specific targets.

## UI Cleanup Workflow

Desktop actions (confirmation required for destructive operations):

- `Archive`: move active skill into runtime archives.
- `Restore`: bring archived skill back as active global skill.
- `Make global`: promote active project skill to global scope.
- `Rename`: normalize skill key from title and move path safely.
- `Delete`: remove active skill (to trash) or remove archived bundle.

`subagents` in current phase are `sync + inspect` (read-only lifecycle).

## Repository Layout

- `platform/crates/agent-sync-core`: shared sync engine
- `platform/crates/agent-sync-cli`: command-line interface
- `platform/apps/agent-sync-desktop`: desktop app (Tauri + React)

## Related Docs

- [dotagents-migration.md](dotagents-migration.md)
- [macos-signing.md](macos-signing.md)
- [../CONTRIBUTING.md](../CONTRIBUTING.md)
