# DD-001: Strict dotagents Mode as Default

## Status
Accepted

## Context
Agent Sync exists to keep declarative agent assets synchronized across tools. Supporting both strict dotagents flows and older lifecycle flows in the same surface created ambiguous behavior, duplicated code paths, and harder contract testing.

## Decision
Agent Sync defaults to strict dotagents behavior and removes legacy lifecycle commands from the supported CLI surface. The CLI, desktop app, and docs all align on the strict contract.

## Consequences
- The supported command surface stays smaller and easier to validate mechanically.
- Migration paths must be explicit when users come from older agent lifecycle setups.
- Future features should extend the strict contract instead of reviving parallel legacy flows.
