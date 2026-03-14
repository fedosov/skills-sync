# DD-001: Desktop Reset Around Bundled dotagents

## Status

Active

## Context

The previous product mixed a desktop app with a custom Rust sync engine, a custom CLI, synthetic state, migration logic, and architecture guards tied to that model. The target product is a desktop-only control plane that tracks vendor `dotagents` behavior closely instead of maintaining a parallel product model.

## Decision

Dotagents Desktop keeps only the Tauri app and the React UI. The desktop backend owns a small local layer for:

- bundled runtime resolution and checksum verification
- command execution and transcript capture
- app settings persistence
- open-path helpers

The app shells into bundled `@sentry/dotagents` 0.10.0 for supported reads and mutations, and the UI presents those results directly.

## Consequences

Good:

- The product becomes easier to reason about.
- Behavior stays close to the pinned vendor CLI.
- The Tauri backend remains native, testable, and app-owned.

Bad:

- Project scope must be explicit because vendor `dotagents` uses process `cwd`.
- UI empty states must explain manual initialization instead of hiding it behind migration helpers.

Neutral:

- The repository contract, docs, and architecture checks must reflect the desktop-only shape.
- Old crates, specs, and migration docs are intentionally removed rather than kept for compatibility.
