# Rename Project to `agent-sync` (Full Rename, Clean Break)

## Summary
Rename the project brand and technical identifiers from `skillssync` / `skills-sync` to `agent-sync` across app UI, CLI, Rust crates, Tauri bundle identity, managed markers, env vars, docs, CI, and GitHub repository metadata.

Chosen decisions:
- Scope: **full stack**
- Compatibility: **clean break** (no alias/migration logic)
- App display name: **Agent Sync Desktop**
- App bundle ID: **dev.fedosov.agent-sync.desktop**
- External markers/env: **rename all**
- Schema `$id`: **keep current URL** (`skillssync.dev`) for now

## Public/API Interface Changes
1. CLI command changes from `skillssync` to `agent-sync`.
2. Rust crate names change:
   - `skillssync-core` -> `agent-sync-core` (import path `agent_sync_core`)
   - `skillssync-cli` -> `agent-sync-cli`
   - `skillssync-desktop` -> `agent-sync-desktop`
3. Desktop app identity changes:
   - Product/window title -> `Agent Sync Desktop`
   - Bundle identifier -> `dev.fedosov.agent-sync.desktop`
4. Environment variables change:
   - `SKILLS_SYNC_GROUP_DIR` -> `AGENT_SYNC_GROUP_DIR`
   - `SKILLS_SYNC_RUNTIME_DIR` -> `AGENT_SYNC_RUNTIME_DIR`
   - `SKILLS_SYNC_DOTAGENTS_BIN` -> `AGENT_SYNC_DOTAGENTS_BIN`
   - `SKILLS_SYNC_DOTAGENTS_BUNDLE_DIR` -> `AGENT_SYNC_DOTAGENTS_BUNDLE_DIR`
5. Managed markers change:
   - `# skills-sync:*` -> `# agent-sync:*` (skills/subagents/MCP blocks)
6. Runtime/storage defaults change:
   - `~/Library/Application Support/SkillsSync` -> `~/Library/Application Support/AgentSync`
   - `~/.skillssync` -> `~/.agent-sync`
   - `~/.config/ai-agents/skillssync` -> `~/.config/ai-agents/agent-sync`

## Assumptions and Defaults
1. Owner stays `fedosov`; only repo slug changes to `agent-sync`.
2. Clean break is intentional: no runtime migration logic, no legacy aliases, no compatibility fallback for old env keys/markers/storage paths.
3. Existing users with old files may need manual cleanup/migration outside app logic.
4. Existing uncommitted local changes in the working tree are unrelated and must be preserved during rename work.

## Test Cases and Verification Scenarios

1. Rust compile/test:
   - `cd /Users/fedosov/Dev/skills-sync/platform && cargo test --workspace`
2. UI test:
   - `cd /Users/fedosov/Dev/skills-sync/platform/apps/agent-sync-desktop/ui && npm run test`
3. Lint full:
   - `cd /Users/fedosov/Dev/skills-sync && make lint`
4. CLI smoke:
   - `cd /Users/fedosov/Dev/skills-sync/platform && cargo run -p agent-sync-cli -- sync --scope all --json`
5. App launch smoke:
   - `cd /Users/fedosov/Dev/skills-sync && ./scripts/run-tauri-gui.sh`
   - Verify visible title is `Agent Sync Desktop`
6. Marker/output verification:
   - Run sync and confirm generated managed blocks contain `# agent-sync:*` only.
7. Config/runtime verification:
   - Confirm app writes runtime state under new `agent-sync` paths and new env vars are honored.
8. CI path verification:
   - Run workflow lint (`actionlint` + yamllint) to confirm renamed directories are referenced correctly.

## Acceptance Criteria
1. No `skillssync` / `skills-sync` references remain in tracked source/docs/config except intentionally kept schema `$id`.
2. CLI command works as `agent-sync`; docs/spec examples match it.
3. Desktop app builds and displays `Agent Sync Desktop` with bundle ID `dev.fedosov.agent-sync.desktop`.
4. Managed config blocks and env vars use `agent-sync` naming.
5. CI workflows and local Make targets operate with renamed paths.
6. GitHub repository slug is `agent-sync` and local remote points to it.

---

## Tasks

### Task 1: Rename directories and update workspace/config paths

- [x] git mv platform/crates/skillssync-core -> platform/crates/agent-sync-core
- [x] git mv platform/crates/skillssync-cli -> platform/crates/agent-sync-cli
- [x] git mv platform/apps/skillssync-desktop -> platform/apps/agent-sync-desktop
- [x] Update platform/Cargo.toml workspace members paths
- [x] Update Makefile APP_DIR, UI_DIR, TAURI_DIR variables
- [x] Update scripts/run-tauri-gui.sh APP_DIR and UI_DIR
- [x] Update .github/workflows/lint.yml paths (cache-dependency-path, working-directory)
- [x] Update .github/workflows/publish.yml paths (cache-dependency-path, working-directory, projectPath, releaseName)
- [x] Update .github/workflows/manual-build-release.yml paths (cache-dependency-path, working-directory, projectPath, releaseName)
- [x] Update .github/dependabot.yml npm directory
- [x] Update .github/labeler.yml ui pattern
- [x] Update .gitignore paths
- [x] Update platform/.gitignore paths
- [x] Update README.md paths and CLI references
- [x] Update docs/SETUP.md paths and CLI references
- [x] Update docs/dotagents-migration.md CLI command references
- [x] Update docs/macos-signing.md path references
- [x] Update AGENTS.md paths and structure references
- [x] Update platform/README.md paths and CLI references

### Task 2: Rename package/crate/bin identifiers in Cargo manifests

- [x] Update platform/crates/agent-sync-core/Cargo.toml package name to agent-sync-core
- [x] Update platform/crates/agent-sync-cli/Cargo.toml package name to agent-sync-cli and bin name to agent-sync
- [x] Update platform/apps/agent-sync-desktop/src-tauri/Cargo.toml package name and dependencies
- [x] Update workspace.package authors in platform/Cargo.toml
- [x] Replace all `use skillssync_core::` with `use agent_sync_core::` in Rust source files
- [x] Replace `extern crate skillssync_core` if any
- [x] Run cargo check to regenerate Cargo.lock with new crate names

### Task 3: Rename app branding and Tauri identity

- [x] Update tauri.conf.json productName to Agent Sync Desktop and identifier to dev.fedosov.agent-sync.desktop
- [x] Update window title in tauri.conf.json to Agent Sync Desktop
- [x] Update ui/index.html title to Agent Sync Desktop
- [x] Update ui/package.json name field
- [x] Update ui/package-lock.json name field
- [x] Update App.tsx any hardcoded SkillsSync brand strings
- [x] Update AppHeader.tsx brand strings
- [x] Update CLAUDE.md app name and path references

### Task 4: Rename runtime keys, paths, env vars, and managed markers

- [x] Update env var names in platform/crates/agent-sync-core/src/paths.rs (SKILLS_SYNC_* -> AGENT_SYNC_*)
- [x] Update default storage paths in paths.rs (SkillsSync -> AgentSync, .skillssync -> .agent-sync)
- [x] Update engine.rs if it references old env vars or paths
- [x] Update dotagents_runtime.rs env var names
- [x] Update managed marker constants in codex_registry.rs (# skills-sync: -> # agent-sync:)
- [x] Update managed marker constants in codex_subagent_registry.rs
- [x] Update managed marker constants in mcp_registry.rs
- [x] Update CLI help text / command metadata in agent-sync-cli/src/main.rs
- [x] Update ui/src/lib/uiSettings.ts localStorage keys
- [x] Update App.tsx localStorage key references
- [x] Update App.test.tsx test expectations for new keys

### Task 5: Update docs/spec/contracts for new command/name

- [ ] Update platform/spec/cli-contract.json CLI command name and examples
- [ ] Verify platform/spec/state.schema.json $id is unchanged (keep skillssync.dev)

### Task 6: GitHub rename and local remote sync

- [ ] Rename GitHub repository fedosov/skills-sync -> fedosov/agent-sync via GitHub web UI or gh CLI
- [ ] Update local git remote: git remote set-url origin https://github.com/fedosov/agent-sync.git
