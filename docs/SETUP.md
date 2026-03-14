# Dotagents Desktop Setup

Dotagents Desktop is a desktop-only Tauri app that wraps bundled `@sentry/dotagents` 0.10.0. The repo intentionally does not ship a custom sync engine, custom CLI, synthetic catalog, or migration layer.

## Prerequisites

- Rust stable toolchain and Cargo
- Node.js 22+ and npm
- Tauri CLI:
  - `cargo install tauri-cli`
- Tauri system dependencies for your OS:
  - [https://v2.tauri.app/start/prerequisites/](https://v2.tauri.app/start/prerequisites/)

## Local development

From the repository root:

```bash
make lint
make test
cd platform/apps/agent-sync-desktop/ui && npm run test
./scripts/run-tauri-gui.sh
```

Manual app startup:

```bash
cd platform/apps/agent-sync-desktop/ui
npm install
cd ../src-tauri
cargo tauri dev
```

## Runtime model

- The app only runs against the bundled `dotagents` runtime.
- Packaged builds must not fall back to a globally installed `dotagents`.
- Dev and test can override the bundled runtime with:
  - `DOTAGENTS_DESKTOP_DOTAGENTS_BIN`
  - `DOTAGENTS_DESKTOP_DOTAGENTS_BUNDLE_DIR`

## Supported desktop actions

Reads:

- `dotagents list --json`
- `dotagents mcp list --json`
- runtime validation through the bundled binary and `--version`

Mutations:

- `dotagents install`
- `dotagents install --frozen`
- `dotagents sync`
- `dotagents add <source> --name <skill>`
- `dotagents add <source> --all`
- `dotagents remove <name>`
- `dotagents update [name]`
- `dotagents mcp add <name> --command ...`
- `dotagents mcp add <name> --url ...`
- `dotagents mcp remove <name>`

Explicitly out of scope for the desktop v1 flow:

- `dotagents init`
- `dotagents doctor`
- `dotagents doctor --fix`
- top-level trust editing

## Scope model

Dotagents Desktop supports only two scopes:

- `user`
- `project`

`user` scope:

- Runs commands with `--user`
- Does not require a project root
- Expects manual initialization under `~/.agents`

`project` scope:

- Uses the selected project folder as command `cwd`
- Never infers a workspace automatically
- Requires a project folder before any project command can run
- Shows an empty state when the selected folder does not contain `agents.toml`

## UI structure

The desktop UI has three tabs:

- `Skills`
- `MCP`
- `Output`

The `Output` tab persists the last command transcript, including command text, `cwd`, scope, exit code, duration, stdout, and stderr.

## Validation shortcuts

From the repository root:

```bash
make lint
make test
make check-arch
```

Direct UI checks:

```bash
cd platform/apps/agent-sync-desktop/ui
npm run test
npm run test:coverage
```

## Repository layout

- `platform/apps/agent-sync-desktop/src-tauri`: Tauri backend
- `platform/apps/agent-sync-desktop/ui`: React + Vite frontend
- `scripts/`: local development helpers

## Related docs

- [design-docs/index.md](design-docs/index.md)
- [macos-signing.md](macos-signing.md)
- [../CONTRIBUTING.md](../CONTRIBUTING.md)
