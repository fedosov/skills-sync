#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

xcodegen generate
xcodebuild -project SkillsSync.xcodeproj -scheme SkillsSyncApp -configuration Debug -allowProvisioningUpdates build

DERIVED_APP="$(find "$HOME/Library/Developer/Xcode/DerivedData" -path '*Build/Products/Debug/SkillsSyncApp.app' -type d | head -n 1)"
TARGET_APP="$HOME/Applications/SkillsSyncApp.app"

if [[ -z "$DERIVED_APP" || ! -d "$DERIVED_APP" ]]; then
  echo "Could not locate built SkillsSyncApp.app in DerivedData" >&2
  exit 1
fi

mkdir -p "$HOME/Applications"
rm -rf "$TARGET_APP"
ditto "$DERIVED_APP" "$TARGET_APP"
open -na "$TARGET_APP"
