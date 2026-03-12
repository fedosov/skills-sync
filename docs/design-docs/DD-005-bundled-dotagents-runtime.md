# DD-005: Bundled dotagents Runtime

## Status
Accepted

## Context
Desktop users should be able to run strict dotagents workflows without depending on a separately installed global runtime or a manually curated PATH.

## Decision
The desktop app bundles and validates the dotagents runtime it needs. Runtime resolution still supports fallbacks, but the bundled runtime is the primary desktop path.

## Consequences
- Desktop onboarding is simpler and more reliable.
- Runtime verification and checksum logic become part of the app’s core responsibilities.
- Release engineering must keep the bundled runtime and validation metadata current.
