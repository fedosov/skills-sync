---
name: update-latest-screenshot
description: Update project screenshot from the latest PNG in ~/Screenshots, publish as docs/images/agent-sync-screenshot-<hash>.png, and update README path. Use when refreshing project screenshots. Invoke with /update-latest-screenshot.
allowed-tools: Bash, Read, Glob
argument-hint: "<repo_root>"
---

# /update-latest-screenshot — Screenshot Updater

Update project screenshot from the latest local capture, optimize, hash-rename, and update README.

## When to Use

- "Update the project screenshot"
- "Refresh the README screenshot"
- "Take latest screenshot and publish it"

## When NOT to Use

- Capturing UI flows or before/after comparisons (use `userflow-screenshots`)
- Creating marketing assets or mockups (use design tools)
- Screenshots for a different project than agent-sync

---

## Workflow

1. Find the latest `.png` in `~/Screenshots`.
2. Copy to `docs/images/`, optimize with `optipng -o3 -strip all` (fallback: `pngquant`).
3. Compute SHA-256, rename to `docs/images/agent-sync-screenshot-<hash12>.png`.
4. Update `README.md` image reference (replace old hash/legacy path).
5. Cleanup old `docs/images/agent-sync-screenshot-*.png` files.
6. Report results.

**Command:**
```bash
/Users/fedosov/.claude/skills/update-latest-screenshot/scripts/update_screenshot.sh <repo_root>
```

## Constraints

- **NEVER** run without a repo_root argument — script requires it
- **NEVER** delete non-managed screenshots (only `agent-sync-screenshot-*.png`)
- **ALWAYS** verify `~/Screenshots` has `.png` files before running
- **ALWAYS** check exit code — non-zero means no screenshots found

## Output Format

```
SCREENSHOT UPDATED
source: {~/Screenshots/filename.png}
target: {docs/images/agent-sync-screenshot-<hash>.png}
size: {before_bytes} → {after_bytes} ({delta})
hash: {sha256_12}
readme: {updated | no change}
cleaned: {N} old file(s) removed
```

## Verification

- [ ] New PNG exists at target path
- [ ] README.md references the new filename
- [ ] No old `agent-sync-screenshot-*.png` files remain (except current)
- [ ] File size is reasonable (>10KB for a real screenshot)
