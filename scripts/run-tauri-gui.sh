#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
UI_DIR="$ROOT_DIR/platform/apps/skillssync-desktop/ui"
TAURI_DIR="$ROOT_DIR/platform/apps/skillssync-desktop/src-tauri"

if ! cargo tauri --help >/dev/null 2>&1; then
  echo "cargo-tauri is not installed. Install it with: cargo install tauri-cli" >&2
  exit 1
fi

if [[ ! -d "$UI_DIR/node_modules" ]]; then
  echo "Installing UI dependencies..."
  (cd "$UI_DIR" && npm install)
fi

cd "$TAURI_DIR"
cargo tauri dev
