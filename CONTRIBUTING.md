# Contributing

## Development Setup

1. Clone the repository.
2. Install Rust stable toolchain.
3. Install Node.js 22+ and npm.
4. Install workflow lint tools:
   - `actionlint` (https://github.com/rhysd/actionlint)
   - `yamllint` (`pip install yamllint`)
5. Install repository git hooks:

```bash
make hooks-install
```

## Local Commands

Run from repository root:

```bash
make lint
```

`make lint` runs the full local lint suite:
- `make lint-workflows`
- `make lint-rust`
- `make lint-ui`

Pre-commit runs these lint groups selectively based on staged files:
- Workflow files (`.github/workflows/*.yml|*.yaml` and `.yamllint.yml`) -> `make lint-workflows`
- Rust files and Cargo manifests under `platform/` -> `make lint-rust`
- UI files under `platform/apps/agent-sync-desktop/ui/` -> `make lint-ui`
- Docs-only changes (`README.md`, anything under `docs/`, or `*.md`) -> skip lint
- Any other staged file type -> fallback to full lint (`make lint-workflows`, `make lint-rust`, `make lint-ui`)

Autofix formatting/lint issues when possible:

```bash
make lint-fix
```

Run tests:

```bash
# Rust tests (from repo root)
make test

# UI tests
cd platform/apps/agent-sync-desktop/ui
npm run test:coverage
```

Run desktop app:

```bash
./scripts/run-tauri-gui.sh
```

## Pull Requests

- Keep PRs focused and small.
- Ensure `make lint` passes.
- Add or update tests for behavior changes.
- Include a concise description of user-visible impact.

## Commit Messages

Use clear, imperative messages with an optional scope, for example:

- `feat(ui): add validation status badge`
- `fix(tauri): handle runtime version mismatch`
