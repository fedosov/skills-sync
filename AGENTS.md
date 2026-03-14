# Agent Instructions

Dotagents Desktop is a desktop-only Tauri wrapper around bundled `@sentry/dotagents` 0.10.0.

## Quick Start

These are the default validation commands for normal changes:

```sh
make lint
make test
cd platform/apps/agent-sync-desktop/ui && npm run test
./scripts/run-tauri-gui.sh
```

## Scope and Precedence

- This file is the repository-local operating contract for AI agents in this project.
- If global/default agent rules conflict with this file, follow this file for this repository.
- `CLAUDE.md` is a compatibility shim that points to this file.

## Repository Map

- `platform/` — Rust workspace root for the desktop app.
- `platform/apps/agent-sync-desktop/src-tauri/` — Tauri backend, runtime resolution, settings, and command execution.
- `platform/apps/agent-sync-desktop/ui/` — React + TypeScript + Vite frontend.
- `docs/` — setup notes, signing docs, and the current architecture decision record.
- `scripts/` — development helpers such as `run-tauri-gui.sh` and `check-architecture.sh`.

## Product Contract

- The app wraps bundled `dotagents`; it does not own a custom sync engine or custom CLI surface.
- The desktop scope model is only `project` or `user`.
- `project` scope requires an explicit selected project folder.
- `user` scope runs vendor commands with `--user`.
- Packaged builds must not fall back to a globally installed `dotagents`.
- `init`, `doctor`, `doctor --fix`, and trust editing are out of the main v1 UI flow.

## Source of Truth

- Prefer executable sources over prose docs when they conflict:
  - Commands: `Makefile`, UI `package.json`, Tauri `src/main.rs`.
  - Runtime behavior: `platform/apps/agent-sync-desktop/src-tauri/src/`.
  - UI contract: `platform/apps/agent-sync-desktop/ui/src/types.ts` and `platform/apps/agent-sync-desktop/ui/src/tauriApi.ts`.
- Treat generated/runtime folders as non-authoritative for design decisions:
  - `platform/target/`
  - UI `dist/`
  - coverage outputs
  - caches

## Environment Requirements

- Rust stable toolchain
- Node.js 22+ and npm
- Tauri CLI (`cargo install tauri-cli`)
- Tauri OS prerequisites from [tauri.app](https://v2.tauri.app/start/prerequisites/)

## Verified Commands

Prefer Makefile targets over raw commands.

| Task | Command |
|---|---|
| Full lint | `make lint` |
| Fix lint | `make lint-fix` |
| Workflow lint | `make lint-workflows` |
| Rust lint only | `make lint-rust` |
| UI lint only | `make lint-ui` |
| Rust tests | `make test` |
| UI tests | `cd platform/apps/agent-sync-desktop/ui && npm run test` |
| UI coverage | `cd platform/apps/agent-sync-desktop/ui && npm run test:coverage` |
| UI e2e tests | `make test-e2e` |
| TS typecheck | `make typecheck-ts` |
| Rust check | `make check-rust` |
| Arch guard | `make check-arch` |
| Run desktop app | `./scripts/run-tauri-gui.sh` |

## Working Rules

- No mandatory post-iteration build/copy step.
- Use Conventional Commits style for commit messages (for example, `fix(ui): preserve output transcript`).
- When borrowing ideas, patterns, or configs from external repositories, append source notes to `INSPIRATIONS.md`.
- Keep changes focused; do not edit unrelated files.
- Add or update tests for behavior changes.
- Run validation appropriate to your change scope before finalizing.
- See `CONTRIBUTING.md` for commit and PR expectations.

## Area-Specific Guidance

### Tauri Backend (`platform/apps/agent-sync-desktop/src-tauri`)

- Keep the command surface aligned with the desktop product contract.
- Reuse only the app-owned helpers in this crate: runtime resolution, command runner, settings, and open-path helpers.
- Do not reintroduce PATH fallback for packaged runtime resolution.
- Keep command results transcript-friendly: always return command, cwd, scope, exit code, duration, stdout, and stderr.

### UI (`platform/apps/agent-sync-desktop/ui`)

- Maintain strict TypeScript quality: avoid `any` and vague `unknown` plumbing where concrete types are available.
- Keep `tauriApi.ts` and `types.ts` synchronized with backend payloads.
- The UI should stay focused on `Skills`, `MCP`, and `Output`.
- Do not reintroduce old product concepts like background polling, synthetic catalog state, archive flows, favorites, rename flows, or audit panes.
- Add or update Vitest/RTL tests for user-visible behavior changes.

## Compatibility Checklist

Before merging changes that affect interfaces or behavior, verify impact in:

- `platform/apps/agent-sync-desktop/src-tauri/src/main.rs`
- `platform/apps/agent-sync-desktop/src-tauri/src/dotagents_runner.rs`
- `platform/apps/agent-sync-desktop/src-tauri/src/dotagents_runtime.rs`
- `platform/apps/agent-sync-desktop/ui/src/types.ts`
- `platform/apps/agent-sync-desktop/ui/src/tauriApi.ts`
- `docs/SETUP.md`

<!-- BEGIN BEADS INTEGRATION -->
## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Git-friendly: Dolt-powered version control with native sync
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

### Quick Start

**Check for ready work:**

```bash
bd ready --json
```

**Create new issues:**

```bash
bd create "Issue title" --description="Detailed context" -t bug|feature|task -p 0-4 --json
bd create "Issue title" --description="What this issue is about" -p 1 --deps discovered-from:bd-123 --json
```

**Claim and update:**

```bash
bd update <id> --claim --json
bd update bd-42 --priority 1 --json
```

**Complete work:**

```bash
bd close bd-42 --reason "Completed" --json
```

### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

### Workflow for AI Agents

1. **Check ready work**: `bd ready` shows unblocked issues
2. **Claim your task atomically**: `bd update <id> --claim`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue:
   - `bd create "Found bug" --description="Details about what was found" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`

### Auto-Sync

bd automatically syncs via Dolt:

- Each write auto-commits to Dolt history
- Use `bd dolt push`/`bd dolt pull` for remote sync
- No manual export/import needed!

### Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Link discovered work with `discovered-from` dependencies
- ✅ Check `bd ready` before asking "what should I work on?"
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems

For more details, see README.md and docs/QUICKSTART.md.

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

<!-- END BEADS INTEGRATION -->
