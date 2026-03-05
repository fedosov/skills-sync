# Design Decisions

Catalog of architectural and product decisions for Agent Sync.

## Active Decisions

| ID | Decision | Date | Status |
|---|---|---|---|
| DD-001 | Strict dotagents mode as default (no legacy lifecycle) | 2025 | Active |
| DD-002 | Scope-based resolution: global > project with conflict detection | 2025 | Active |
| DD-003 | Symlink-based sync (canonical source + managed symlinks to targets) | 2025 | Active |
| DD-004 | Managed blocks in config files for non-symlink targets (config.toml) | 2025 | Active |
| DD-005 | Bundled dotagents runtime in desktop app (no global install required) | 2025 | Active |
| DD-006 | Workspace discovery from ~/Dev + configured roots, exclude worktrees | 2025 | Active |

## How to Add

Create a new file `docs/design-docs/DD-NNN-title.md` with:
- Context and problem statement
- Decision and rationale
- Consequences (positive and negative)
- Update this index
