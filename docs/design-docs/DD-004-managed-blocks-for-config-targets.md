# DD-004: Managed Blocks for Config Targets

## Status
Accepted

## Context
Some targets, especially agent config files, cannot be managed as standalone symlinks without clobbering unrelated user-managed content.

## Decision
Agent Sync writes only the managed sections of those files using explicit begin/end markers. Unmanaged content outside the markers is preserved, and mutation helpers operate only inside the managed block.

## Consequences
- The app can coexist with user-authored config in the same file.
- Rendering logic must preserve stable formatting and marker placement.
- Validation and cleanup paths need to understand both managed and unmanaged sections.
