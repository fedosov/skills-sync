# Dotagents Desktop

A Tauri desktop app that wraps two independent agent-skill management CLIs (`@sentry/dotagents` and `skills`) behind a single shell, giving each its own UI workspace.

## Language

**Workspace**:
A self-contained sub-app inside the desktop shell that fronts exactly one CLI. The app currently has two: **Dotagents Workspace** and **Skills Workspace**. Each workspace owns its own sidebar, command set, scope state, active-agent state, and Output transcript.
_Avoid_: Tab, panel, view (those are sub-elements within a workspace)

**Workspace Switcher**:
The top-level header control that toggles between **Dotagents Workspace** and **Skills Workspace**. Manual selection only — no auto-detect from cwd.

**Dotagents CLI**:
`@sentry/dotagents`, pinned to `1.4.0`, invoked via `npx`. Manages skills via `agents.toml` + `agents.lock`, plus MCP and hooks. Owned by the Dotagents Workspace.

**Skills CLI**:
The `skills` package on npm (`npx skills@latest cli`). Manages skills as packages installed into per-agent directories (`~/.claude/skills/`, `~/.cursor/...`) with optional `skills-lock.json`. Owned by the Skills Workspace.

**Scope** (skills/dotagents):
`global` or `project`. Per-workspace selector — each workspace remembers its own scope. Default is chosen by a cwd heuristic (signs of a project repo → `project`, else `global`); user can override.

**Active Agents**:
The set of target agents (`Claude Code`, `Cursor`, `Codex`, …) that the Skills Workspace applies its commands to. Multi-select, per-workspace. Initial value populated by scanning standard agent directories; user confirms / edits.

**Skill Source**:
The origin of an installed skill (typically a GitHub repo like `vercel-labs/agent-skills`). Tracked by Skills CLI in `skills-lock.json` only — `skills list --json` does NOT return it. Surfaced in UI by reading the lock file separately and merging by skill name. Distinct from a skill's on-disk `path`.

**Skill Status** (dotagents only):
`Ok | Modified | Missing | Unlocked`. A drift-detection concept native to Dotagents (which content-hashes lock entries). Skills CLI has no equivalent — the Skills Workspace does NOT show a status column.

## Relationships

- A **Workspace** owns its own **Scope**, **Active Agents** (Skills only), and Output transcript; switching workspaces preserves each workspace's state.
- The **Skills Workspace** combines three data sources to render its skill list: `skills list --json` (name, path, scope, agents), `skills-lock.json` (source, version), and each skill's `SKILL.md` frontmatter (description). All merged by skill `name` in `skills_runner.rs`.
- **Active Agents** values come from `skills list --json` as display names ("Claude Code"); the `--agent` CLI flag expects kebab-case ("claude-code"). The runner maintains an explicit display↔kebab mapping.

## Flagged ambiguities

- "Skills" is overloaded: it can mean the **Skills CLI** (a specific tool), an installed **skill** (one package), or the **Skills Workspace** (the UI surface). Always qualify when the surrounding sentence is ambiguous.
- "Agent" can mean a **target agent** (Claude Code, Cursor — a consumer of skills) or an **AI agent** (Claude, the user's coding assistant). In this repo, "agent" defaults to the former unless explicitly noted.
