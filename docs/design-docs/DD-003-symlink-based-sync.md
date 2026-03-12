# DD-003: Symlink-based Sync

## Status
Accepted

## Context
Most managed agent assets are file or directory trees that need to stay editable at a canonical source while appearing inside target agent directories.

## Decision
Agent Sync treats the canonical source as authoritative and reconciles managed targets through symlinks whenever the target platform supports it. The sync engine records the resolved targets in state and repairs them during reconciliation.

## Consequences
- Canonical edits stay visible immediately across managed targets.
- Drift detection and repair logic can stay centralized in the sync engine.
- Some target formats still need a fallback path when symlinks are not viable.
