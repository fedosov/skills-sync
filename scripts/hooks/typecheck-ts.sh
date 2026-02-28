#!/bin/bash
INPUT=$(cat 2>/dev/null || echo '{}')
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null)

if [[ ! "$FILE_PATH" =~ \.(ts|tsx)$ ]]; then
  exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-.}/platform/apps/agent-sync-desktop/ui" || exit 0
npx tsc --noEmit 2>&1 | tail -20
