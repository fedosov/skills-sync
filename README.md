# Skills Sync App

Native macOS app for monitoring and controlling skills sync.

## Features

- Sync health (`OK/FAILED/SYNCING`) + counts
- Host app with searchable full skill list
- Actions via command queue:
  - Open in Zed
  - Reveal in Finder
  - Delete canonical source (move to Trash)

## Data contract

- Primary app state file: `~/Library/Group Containers/group.dev.fedosov.skillssync/state.json`
- Primary app command queue: `~/Library/Group Containers/group.dev.fedosov.skillssync/commands.jsonl`
- Backend runtime mirror: `/Users/fedosov/.config/ai-agents/skillssync/state.json`

## Build

```bash
cd /Users/fedosov/Dev/ai-skills-widget
xcodegen generate
xcodebuild -project SkillsSync.xcodeproj -scheme SkillsSyncApp -configuration Debug -allowProvisioningUpdates build
xcodebuild -project SkillsSync.xcodeproj -scheme SkillsSyncApp -destination 'platform=macOS' -allowProvisioningUpdates test
```

## Notes

- Backend authority remains `~/.config/ai-agents/sync-skills.py`.
- UI only appends commands and renders state.
- Delete action requires explicit confirmation in host app.
