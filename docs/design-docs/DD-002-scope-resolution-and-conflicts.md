# DD-002: Scope Resolution and Conflict Detection

## Status
Accepted

## Context
Skills, subagents, and MCP servers can exist at both global and project scopes. Without deterministic precedence and conflict reporting, synchronized results become surprising and difficult to debug.

## Decision
Agent Sync resolves assets by scope with explicit conflict detection between competing sources. Global and project assets remain visible in state, and conflicting entries are surfaced instead of being silently overwritten.

## Consequences
- Sync behavior stays deterministic across CLI and desktop workflows.
- Users get explicit diagnostics when two scopes disagree.
- New catalog mutations and migration flows must preserve the same precedence and conflict semantics.
