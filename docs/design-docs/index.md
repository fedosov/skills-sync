# Design Decisions

Catalog of architectural and product decisions for Agent Sync.

## Active Decisions

| ID | Decision | Date | Status |
|---|---|---|---|
| DD-001 | [Strict dotagents mode as default (no legacy lifecycle)](DD-001-strict-dotagents-default.md) | 2025 | Active |
| DD-002 | [Scope-based resolution: global > project with conflict detection](DD-002-scope-resolution-and-conflicts.md) | 2025 | Active |
| DD-003 | [Symlink-based sync (canonical source + managed symlinks to targets)](DD-003-symlink-based-sync.md) | 2025 | Active |
| DD-004 | [Managed blocks in config files for non-symlink targets (config.toml)](DD-004-managed-blocks-for-config-targets.md) | 2025 | Active |
| DD-005 | [Bundled dotagents runtime in desktop app (no global install required)](DD-005-bundled-dotagents-runtime.md) | 2025 | Active |
| DD-006 | [Workspace discovery from ~/Dev + configured roots, exclude worktrees](DD-006-workspace-discovery-rules.md) | 2025 | Active |
| DD-007 | [Tauri runtime ownership and auto-watch coordination](DD-007-tauri-runtime-and-auto-watch.md) | 2026 | Active |
| DD-008 | [Architecture boundaries and file splitting guardrails](DD-008-architecture-boundaries-and-file-splitting.md) | 2026 | Active |

## How to Add

Create a new file `docs/design-docs/DD-NNN-title.md` with:
- Context and problem statement
- Decision and rationale
- Consequences (positive and negative)
- Update this index
