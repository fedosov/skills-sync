#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <repo_root>" >&2
  exit 1
fi

REPO_ROOT="$(cd "$1" && pwd -P)"
SCREENSHOT_DIR="$HOME/Screenshots"
IMAGES_DIR="$REPO_ROOT/docs/images"
LEGACY_TARGET="$IMAGES_DIR/agent-sync-latest-screenshot.png"
README_FILE="$REPO_ROOT/README.md"
TMP_TARGET="$IMAGES_DIR/.agent-sync-screenshot-tmp-$$.png"

cleanup_tmp() {
  if [[ -f "$TMP_TARGET" ]]; then
    rm -f "$TMP_TARGET"
  fi
}
trap cleanup_tmp EXIT

file_hash() {
  local file="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
    return
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
    return
  fi
  echo "no SHA-256 utility available (expected shasum or sha256sum)" >&2
  exit 1
}

if [[ ! -d "$SCREENSHOT_DIR" ]]; then
  echo "screenshots directory not found: $SCREENSHOT_DIR" >&2
  exit 1
fi

LATEST_FILE=""
LATEST_MTIME=0
while IFS= read -r -d '' candidate; do
  mtime="$(stat -f %m "$candidate" 2>/dev/null || echo 0)"
  if (( mtime > LATEST_MTIME )); then
    LATEST_MTIME="$mtime"
    LATEST_FILE="$candidate"
  fi
done < <(
  find "$SCREENSHOT_DIR" -type f -iname '*.png' -print0
)

if [[ -z "$LATEST_FILE" ]]; then
  echo "no png images found in $SCREENSHOT_DIR" >&2
  exit 1
fi

mkdir -p "$IMAGES_DIR"

previous_target_size=0
shopt -s nullglob
for existing in "$IMAGES_DIR"/agent-sync-screenshot-*.png "$LEGACY_TARGET"; do
  if [[ -f "$existing" ]]; then
    previous_target_size=$(stat -f%z "$existing")
    break
  fi
done
shopt -u nullglob

cp "$LATEST_FILE" "$TMP_TARGET"
before_optimize_size=$(stat -f%z "$TMP_TARGET")

if command -v optipng >/dev/null 2>&1; then
  optipng -o3 -strip all "$TMP_TARGET" >/dev/null
elif command -v pngquant >/dev/null 2>&1; then
  pngquant --skip-if-larger --force --output "$TMP_TARGET" "$TMP_TARGET" >/dev/null
else
  echo "warning: neither optipng nor pngquant found; file copied without optimization" >&2
fi

full_hash="$(file_hash "$TMP_TARGET")"

image_hash="${full_hash:0:12}"
TARGET="$IMAGES_DIR/agent-sync-screenshot-${image_hash}.png"
mv -f "$TMP_TARGET" "$TARGET"
after_size=$(stat -f%z "$TARGET")
readme_updated=false

if [[ -f "$README_FILE" ]]; then
  if rg -q 'docs/images/(agent-sync-latest-screenshot\.png(\?v=[0-9]{8}-[0-9]{6})?|agent-sync-screenshot-[0-9a-f]{12}\.png)' "$README_FILE"; then
    before_readme_hash="$(file_hash "$README_FILE")"
    sed -E -i '' \
      "s#docs/images/(agent-sync-latest-screenshot\.png(\\?v=[0-9]{8}-[0-9]{6})?|agent-sync-screenshot-[0-9a-f]{12}\\.png)#docs/images/agent-sync-screenshot-${image_hash}.png#g" \
      "$README_FILE"
    after_readme_hash="$(file_hash "$README_FILE")"
    if [[ "$before_readme_hash" != "$after_readme_hash" ]]; then
      readme_updated=true
    fi
  fi
fi

deleted_old_files=0
shopt -s nullglob
for candidate in "$IMAGES_DIR"/agent-sync-screenshot-*.png; do
  if [[ "$candidate" != "$TARGET" ]]; then
    rm -f "$candidate"
    deleted_old_files=$((deleted_old_files + 1))
  fi
done
shopt -u nullglob

if [[ -f "$LEGACY_TARGET" ]]; then
  rm -f "$LEGACY_TARGET"
  deleted_old_files=$((deleted_old_files + 1))
fi

echo "source=$LATEST_FILE"
echo "target=$TARGET"
echo "previous_target_bytes=$previous_target_size"
echo "before_bytes=$before_optimize_size"
echo "after_bytes=$after_size"
echo "image_hash=$image_hash"
echo "readme=$README_FILE"
echo "readme_updated=$readme_updated"
echo "deleted_old_files=$deleted_old_files"
