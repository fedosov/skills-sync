# Product Specs

## v1 Desktop App

- Wraps pinned `@sentry/dotagents` v1.4.0 via npx
- Two scopes: `project` (selected folder) and `user` (global)
- UI focused on: Skills, MCP, Output
- Out of scope for v1: `init`, `doctor`, `doctor --fix`, trust editing

## Anti-Patterns (do not reintroduce)

- Background polling
- Synthetic catalog state
- Archive flows
- Favorites / rename flows
- Audit panes
