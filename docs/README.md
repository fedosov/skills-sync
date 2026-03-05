# Documentation Index

Entry point for all Agent Sync documentation. Start with [AGENTS.md](../AGENTS.md) for the operating contract.

## Docs

| Document | Description |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Domain map, layer ordering, dependency rules, key contracts |
| [SETUP.md](SETUP.md) | Prerequisites, build instructions, platform-specific notes |
| [design-docs/index.md](design-docs/index.md) | Architectural decision catalog (DD-001 through DD-006) |
| [macos-signing.md](macos-signing.md) | Code signing and notarization for macOS builds |
| [dotagents-migration.md](dotagents-migration.md) | Migration guide from dotagents to Agent Sync |

## Contracts and Specs

All machine-readable contracts live in [`platform/spec/`](../platform/spec/):

- `cli-contract.json` — CLI command surface and flags
- `state.schema.json` — Persistent state schema
- `capability-matrix.json` — Feature capabilities per agent
