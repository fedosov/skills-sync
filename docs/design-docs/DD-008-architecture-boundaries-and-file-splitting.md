# DD-008: Architecture Boundaries and File-splitting Guardrails

## Status
Accepted

## Context
The repository documents clear boundaries between core, Tauri, and UI layers, but large files and ad hoc imports can erode that structure over time unless the boundaries are enforced mechanically.

## Decision
Agent Sync enforces dependency direction with executable tests and lint rules, and treats oversized orchestration files as refactoring triggers. Shared UI types live below hooks and components, and Tauri command modules rely on helper/service seams rather than duplicating coordination logic.

## Consequences
- Architectural drift becomes visible in CI instead of being discovered only during reviews.
- Refactors should prefer extracting helper/service seams over growing root orchestration files.
- Adding a new boundary or layer implies updating both the documentation and the executable checks.
