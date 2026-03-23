# Design Docs

Catalog of architectural decisions for Dotagents Desktop.

## Active Decisions

| Decision | Status | Summary |
|----------|--------|---------|
| npx-pinned runtime | Adopted | Use `npx @sentry/dotagents@1.4.0` instead of bundled binary |
| Desktop-only scope | Adopted | No CLI, no library crate — single Tauri desktop app |
| Transcript-friendly results | Adopted | All command results include: command, cwd, scope, exit code, duration, stdout, stderr |
