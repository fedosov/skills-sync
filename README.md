# Dotagents Desktop

Dotagents Desktop is a desktop-only Tauri wrapper around bundled [`@sentry/dotagents` 0.10.0](https://www.npmjs.com/package/@sentry/dotagents/v/0.10.0). The app does not ship a custom sync engine, custom CLI, synthetic catalog, or migration layer. It exposes the vendor behavior directly with an explicit project-or-user context.

## What it does

- Runs only against the bundled `dotagents` runtime.
- Supports vendor reads for `list --json`, `mcp list --json`, and runtime `--version`.
- Supports vendor mutations for `install`, `install --frozen`, `sync`, `add`, `remove`, `update`, `mcp add`, and `mcp remove`.
- Keeps project scope explicit: the app only runs project commands after you pick a project folder.

## Quick start

```bash
make lint
make test
cd platform/apps/agent-sync-desktop/ui && npm run test
./scripts/run-tauri-gui.sh
```

## Product notes

- `project` scope uses the selected folder as command `cwd`.
- `user` scope runs with `--user` and no project root.
- `init`, `doctor`, trust editing, and PATH fallback are intentionally out of the v1 desktop flow.

More setup and operational details live in [docs/SETUP.md](docs/SETUP.md).
