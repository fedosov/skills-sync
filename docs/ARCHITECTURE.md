# Architecture

## Domain Map

Agent Sync manages declarative agent configurations (skills, subagents, MCP servers)
across multiple AI coding agents (Claude, Codex) with scope-based resolution (global/project).

### Business Domains

| Domain | Responsibility | Key modules |
|---|---|---|
| **Sync Engine** | Discovery, conflict detection, symlink reconciliation | `engine.rs` |
| **Skill Registry** | Codex skills manifest read/write | `codex_registry.rs` |
| **Subagent Registry** | Codex subagent config read/write | `codex_subagent_registry.rs` |
| **MCP Registry** | MCP server catalog across agents | `mcp_registry.rs` |
| **dotagents Adapter** | Migration and runtime bridge to dotagents | `dotagents_adapter.rs`, `dotagents_runtime.rs` |
| **State & Audit** | Persistent sync state and audit log | `state_store.rs`, `audit_store.rs` |
| **Catalog Mutations** | Archive, restore, delete, rename, make-global (desktop UI only, not in strict CLI) | `engine.rs` (mutation methods) |
| **Watch** | Filesystem-triggered continuous sync | `watch.rs` |

## Layer Ordering

Dependency flows top-to-bottom only. No reverse imports.

```
Types & Models      (models.rs, error.rs)
     |
Paths & Config      (paths.rs, settings.rs, managed_block.rs)
     |
Registries          (codex_registry, codex_subagent_registry, mcp_registry)
     |
Adapters            (dotagents_adapter, dotagents_runtime, agents_context)
     |
Engine              (engine.rs — orchestrates registries + adapters)
     |
Persistence         (state_store, audit_store)
     |
Watch               (watch.rs — drives engine on fs events)
     |
CLI                 (agent-sync-cli — thin clap wrapper over engine)
     |
Tauri Commands      (src-tauri/commands/* — thin wrappers over engine)
     |
UI                  (React + TypeScript — calls Tauri commands)
```

## Cross-Cutting Concerns

- **Scope resolution**: `ScopeFilter` and `DotagentsScope` enums used across all layers.
- **Error handling**: `SyncEngineError` is the single error type for core; Tauri commands map to string errors.
- **Serialization**: All models use `serde` with `rename_all = "snake_case"` convention.
- **Filesystem access**: Centralized through `SyncPaths` — no ad-hoc path construction.

## Key Contracts

| Contract | Location | Purpose |
|---|---|---|
| CLI contract | `platform/spec/cli-contract.json` | Defines CLI commands, flags, output shapes |
| State schema | `platform/spec/state.schema.json` | JSON schema for `SyncState` |
| Capability matrix | `platform/spec/capability-matrix.json` | Platform/feature support grid |
| UI types | `ui/src/types.ts` | TypeScript mirror of Rust models |
| Tauri API | `ui/src/tauriApi.ts` | Typed invoke wrappers for Tauri commands |

## Dependency Direction Rules

1. `agent-sync-core` has zero dependencies on `agent-sync-cli` or Tauri.
2. `agent-sync-cli` depends only on `agent-sync-core`.
3. Tauri backend depends on `agent-sync-core` (same workspace).
4. UI depends on Tauri invoke API only — never imports Rust types directly.
5. Within `agent-sync-core`, `engine.rs` may import any module; leaf modules (`models`, `paths`, `error`) import nothing from the crate.

## Sync Pipeline (Simplified)

```
discover skills/subagents/MCP in filesystem
    -> resolve scopes (global vs project workspaces)
    -> detect conflicts (same key, different content)
    -> reconcile symlinks to target agent directories
    -> update managed blocks in config files
    -> persist state + audit log
```
