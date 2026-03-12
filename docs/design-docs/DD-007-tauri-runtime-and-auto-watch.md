# DD-007: Tauri Runtime Ownership and Auto-watch Coordination

## Status
Accepted

## Context
The desktop backend needs a single place to coordinate sync locking, runtime toggles, and filesystem watch lifecycles. Spreading lock management across command handlers increases identity coupling and makes watch behavior harder to reason about.

## Decision
The Tauri backend owns runtime coordination through a single `AppRuntime` service. Command handlers delegate locking and watch lifecycle transitions to that runtime instead of manipulating shared lock state directly.

## Consequences
- Command modules stay thinner and less coupled to shared mutable state.
- Watch startup, shutdown, and write-mode transitions are easier to test in one place.
- Future command additions should use the runtime helpers instead of reintroducing direct lock management.
