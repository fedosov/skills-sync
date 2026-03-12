# DD-006: Workspace Discovery Rules

## Status
Accepted

## Context
Project-scoped agent assets only work if Agent Sync can discover the right workspaces consistently while avoiding transient or misleading directories such as worktrees.

## Decision
Workspace discovery starts from configured roots and the conventional `~/Dev` workspace area, while explicitly excluding worktrees and other paths that should not participate in project scope resolution.

## Consequences
- Project scope behavior stays predictable across machines.
- Discovery rules are opinionated and must be documented when they change.
- Future workspace-source additions should preserve the exclusion rules for worktrees and non-project directories.
