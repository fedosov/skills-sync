# dotagents Migration and Rollback

## Scope

SkillsSync strict mode requires `agents.toml` contracts for both user and project scopes before `sync` can run.

## Migration

1. Backup current skill roots (`~/.claude/skills`, `~/.agents/skills`, workspace `.agents/skills`).
2. Ensure `dotagents` is installed and available on `PATH`:
   - `npm install -g @sentry/dotagents@0.10.0`
3. Initialize strict contracts:
   - user scope: `skillssync migrate-dotagents --scope user`
   - project scope: `skillssync migrate-dotagents --scope project`
4. Run strict sync:
   - `skillssync sync --scope all --json`
5. Verify lock-integrity install:
   - `skillssync skills install --scope all`
6. Inspect resulting declarations:
   - `skillssync skills list --scope all --json`
   - `skillssync mcp list --scope all --json`

## Failure Behavior

- Missing user `agents.toml` returns `MigrationRequired` with remediation.
- Legacy project roots without `agents.toml` return `MigrationRequired` with workspace list.
- Bundled binary checksum mismatch returns `DotagentsChecksumMismatch` and blocks command execution.

## Rollback

1. Restore backed-up skill directories and `agents.toml` files.
2. Remove generated strict contracts if needed:
   - user: `~/.agents/agents.toml`
   - project: `<workspace>/agents.toml`
3. Re-run legacy sync pipeline (if enabled in your deployment) to rebuild managed links.
