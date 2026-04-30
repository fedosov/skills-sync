# Skills CLI lives as a separate workspace, not unified with Dotagents

Adding support for `npx skills@latest cli` alongside the existing `@sentry/dotagents` UI. We chose **two side-by-side workspaces** (separate sidebar, command set, scope state, active-agents state, Output transcript, and Rust types per CLI) rather than a unified skill model with a dual backend. The two CLIs have materially different domain models — Dotagents has `agents.toml`/`agents.lock`, MCP, hooks, and content-hash drift detection; Skills CLI has multi-agent install targets (`--agent claude-code cursor`), agent-display-name vs kebab-case duality, and a much thinner `list --json` (only `name`/`path`/`scope`/`agents`). A unified model would have to lie in one direction or the other and would couple two independently evolving CLIs into one type tree.

## Consequences

- A new `skills_runner.rs` lives next to `dotagents_runner.rs`; a new `SkillsCliListItem` type lives next to `DotagentsSkillListItem` rather than reusing it.
- The Skills Workspace enriches its list by merging `skills list --json` with `skills-lock.json` and `SKILL.md` frontmatter, because the CLI's own JSON output omits source, version, and description.
- "Skill Status" (Ok/Modified/Missing/Unlocked) stays a Dotagents-only concept; the Skills Workspace does not have it.
