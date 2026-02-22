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

`make lint` runs the same three lint groups enforced by pre-commit:
- `make lint-workflows`
- `make lint-rust`
- `make lint-ui`

Autofix formatting/lint issues when possible:

```bash
make lint-fix
```

Run tests:

```bash
cd platform
cargo test

cd apps/skillssync-desktop/ui
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
- `fix(core): handle missing skill metadata`
