# Repository Hygiene

Scan for tech debt and track cleanup with [stringer](https://github.com/davetashner/stringer) + [beads](https://github.com/steveyegge/beads).

```bash
$ stringer scan . --max-issues 5 -f markdown > /tmp/stringer-issues.md
# scan for issues in the project (no AI!)

$ bd create -f /tmp/stringer-issues.md
# import issues into beads

$ cc/oc/etc: plan next bead
# pick up a bead and fix it
```

That's the workflow if you want to chip away at tech debt.
