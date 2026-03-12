---
name: release-notes
description: Generate changelog from conventional commits since last git tag, grouped by type (feat/fix/refactor/etc). Invoke with /release-notes.
disable-model-invocation: true
allowed-tools: Bash, Read
---

# /release-notes — Changelog Generator

Generate a formatted changelog from conventional commits since the last release tag.

## When to Use

- Before cutting a new release
- "What changed since the last release?"
- "Generate release notes"

## Workflow

1. **Find the latest tag:**
   ```bash
   git tag --sort=-v:refname | head -1
   ```

2. **Collect commits since that tag:**
   ```bash
   git log <tag>..HEAD --oneline --no-merges
   ```

3. **Parse and group** by Conventional Commits prefix:
   - **Features** (`feat`): new functionality
   - **Fixes** (`fix`): bug fixes
   - **Refactoring** (`refactor`): code restructuring
   - **Docs** (`docs`): documentation changes
   - **Chores** (`chore`): maintenance, deps, tooling
   - **Other**: anything that doesn't match a prefix

4. **Format** as markdown changelog:

```markdown
## What's Changed since <tag>

### Features
- <scope>: <description> (<short-hash>)

### Fixes
- <scope>: <description> (<short-hash>)

### Refactoring
- <description> (<short-hash>)

...
```

5. **Include stats**: total commit count, contributor list (from `git shortlog`).

## Constraints

- **ONLY** include commits between the latest tag and HEAD
- **SKIP** merge commits (`--no-merges`)
- **PRESERVE** scope from commit messages (e.g., `fix(ui):` → listed under Fixes with `ui` scope)
- **DO NOT** fabricate or embellish commit descriptions — use the actual message
- If there are zero commits since the last tag, report that explicitly
