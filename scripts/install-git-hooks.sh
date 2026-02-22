#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOK_PATH=".githooks"
PRE_COMMIT_PATH="$ROOT_DIR/$HOOK_PATH/pre-commit"

if [[ ! -f "$PRE_COMMIT_PATH" ]]; then
  echo "pre-commit hook not found: $PRE_COMMIT_PATH" >&2
  exit 1
fi

chmod +x "$PRE_COMMIT_PATH"
git -C "$ROOT_DIR" config core.hooksPath "$HOOK_PATH"

echo "Installed git hooks from $HOOK_PATH"
echo "Verify with: git config --get core.hooksPath"
