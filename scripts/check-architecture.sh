#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
EXIT_CODE=0

echo "=== Desktop-only workspace check ==="
if ! grep -q 'apps/agent-sync-desktop/src-tauri' "$ROOT_DIR/platform/Cargo.toml"; then
  echo "DRIFT: platform/Cargo.toml must include the desktop Tauri app workspace member"
  EXIT_CODE=1
fi

for removed_path in \
  "$ROOT_DIR/platform/crates/agent-sync-core" \
  "$ROOT_DIR/platform/crates/agent-sync-cli" \
  "$ROOT_DIR/platform/spec"
do
  if [[ -e "$removed_path" ]]; then
    echo "DRIFT: deleted reset paths must stay removed: $removed_path"
    EXIT_CODE=1
  fi
done

echo ""
echo "=== Bundled runtime guard ==="
if grep -q 'bundled dotagents binary not found; packaged runtime is required' \
  "$ROOT_DIR/platform/apps/agent-sync-desktop/src-tauri/src/dotagents_runtime.rs"; then
  echo "  Bundled runtime requirement is enforced."
else
  echo "DRIFT: dotagents_runtime.rs must require the bundled runtime"
  EXIT_CODE=1
fi

echo ""
echo "=== Product metadata check ==="
if grep -q '"productName": "Dotagents Desktop"' \
  "$ROOT_DIR/platform/apps/agent-sync-desktop/src-tauri/tauri.conf.json"; then
  echo "  Tauri metadata uses Dotagents Desktop."
else
  echo "DRIFT: tauri.conf.json product metadata must be renamed to Dotagents Desktop"
  EXIT_CODE=1
fi

echo ""
if [[ "$EXIT_CODE" -ne 0 ]]; then
  echo "Architecture checks found issues."
else
  echo "All architecture checks passed."
fi

exit "$EXIT_CODE"
