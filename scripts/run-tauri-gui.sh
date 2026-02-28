#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$ROOT_DIR/platform/apps/agent-sync-desktop"
UI_DIR="$ROOT_DIR/platform/apps/agent-sync-desktop/ui"

if ! cargo tauri --help >/dev/null 2>&1; then
  echo "cargo-tauri is not installed. Install it with: cargo install tauri-cli" >&2
  exit 1
fi

if [[ ! -d "$UI_DIR/node_modules" ]]; then
  echo "Installing UI dependencies..."
  (cd "$UI_DIR" && npm install)
fi

cd "$APP_DIR"
cargo tauri dev
