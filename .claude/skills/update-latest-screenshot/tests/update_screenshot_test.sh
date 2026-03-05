#!/usr/bin/env bash
set -euo pipefail

SCRIPT="/Users/fedosov/.claude/skills/update-latest-screenshot/scripts/update_screenshot.sh"

decode_b64() {
  local out="$1"
  local payload="$2"
  if base64 -D >/dev/null 2>&1 <<<"AA=="; then
    base64 -D <<<"$payload" >"$out"
  else
    base64 -d <<<"$payload" >"$out"
  fi
}

get_kv() {
  local file="$1"
  local key="$2"
  awk -F= -v k="$key" '$1 == k { print substr($0, length(k) + 2) }' "$file" | tail -n1
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local message="$3"
  if [[ "$expected" != "$actual" ]]; then
    echo "ASSERT FAILED: $message" >&2
    echo "  expected: $expected" >&2
    echo "  actual:   $actual" >&2
    exit 1
  fi
}

assert_file_exists() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    echo "ASSERT FAILED: expected file to exist: $file" >&2
    exit 1
  fi
}

assert_file_not_exists() {
  local file="$1"
  if [[ -f "$file" ]]; then
    echo "ASSERT FAILED: expected file to not exist: $file" >&2
    exit 1
  fi
}

assert_contains() {
  local needle="$1"
  local file="$2"
  if ! rg -q --fixed-strings "$needle" "$file"; then
    echo "ASSERT FAILED: expected '$needle' in $file" >&2
    exit 1
  fi
}

assert_not_contains() {
  local needle="$1"
  local file="$2"
  if rg -q --fixed-strings "$needle" "$file"; then
    echo "ASSERT FAILED: expected '$needle' to be absent in $file" >&2
    exit 1
  fi
}

TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

HOME_FIXTURE="$TMP_ROOT/home"
REPO_FIXTURE="$TMP_ROOT/repo"
mkdir -p "$HOME_FIXTURE/Screenshots" "$REPO_FIXTURE/docs/images"

cat >"$REPO_FIXTURE/README.md" <<'EOF'
# SkillsSync
![SkillsSync screenshot](docs/images/agent-sync-latest-screenshot.png?v=20260221-123317)
EOF

# Keep a legacy file so cleanup behavior is asserted.
decode_b64 "$REPO_FIXTURE/docs/images/agent-sync-latest-screenshot.png" \
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAAAAAA6fptVAAAACklEQVR4nGNgAAAAAgABSK+kcQAAAABJRU5ErkJggg=="

decode_b64 "$HOME_FIXTURE/Screenshots/old.png" \
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAAAAAA6fptVAAAACklEQVR4nGNgAAAAAgABSK+kcQAAAABJRU5ErkJggg=="
touch -t 202602200101 "$HOME_FIXTURE/Screenshots/old.png"

OUT1="$TMP_ROOT/out1.txt"
HOME="$HOME_FIXTURE" "$SCRIPT" "$REPO_FIXTURE" >"$OUT1"

TARGET1="$(get_kv "$OUT1" "target")"
HASH1="$(get_kv "$OUT1" "image_hash")"
UPDATED1="$(get_kv "$OUT1" "readme_updated")"
assert_eq "true" "$UPDATED1" "legacy README should be updated"
assert_file_exists "$TARGET1"
assert_contains "docs/images/agent-sync-screenshot-$HASH1.png" "$REPO_FIXTURE/README.md"
assert_not_contains "?v=" "$REPO_FIXTURE/README.md"
assert_file_not_exists "$REPO_FIXTURE/docs/images/agent-sync-latest-screenshot.png"
assert_eq "1" "$(find "$REPO_FIXTURE/docs/images" -maxdepth 1 -type f -name 'agent-sync-screenshot-*.png' | wc -l | tr -d ' ')" "one managed hash file after first run"

OUT2="$TMP_ROOT/out2.txt"
HOME="$HOME_FIXTURE" "$SCRIPT" "$REPO_FIXTURE" >"$OUT2"
TARGET2="$(get_kv "$OUT2" "target")"
HASH2="$(get_kv "$OUT2" "image_hash")"
assert_eq "$TARGET1" "$TARGET2" "same input should keep same hashed target"
assert_eq "$HASH1" "$HASH2" "same input should keep same hash"
assert_eq "1" "$(find "$REPO_FIXTURE/docs/images" -maxdepth 1 -type f -name 'agent-sync-screenshot-*.png' | wc -l | tr -d ' ')" "still one managed hash file after second run"

sips -z 2 1 "$HOME_FIXTURE/Screenshots/old.png" --out "$HOME_FIXTURE/Screenshots/new.png" >/dev/null
touch -t 202602210101 "$HOME_FIXTURE/Screenshots/new.png"

OUT3="$TMP_ROOT/out3.txt"
HOME="$HOME_FIXTURE" "$SCRIPT" "$REPO_FIXTURE" >"$OUT3"
TARGET3="$(get_kv "$OUT3" "target")"
assert_file_exists "$TARGET3"
if [[ "$TARGET3" == "$TARGET1" ]]; then
  echo "ASSERT FAILED: changed PNG should produce a different hash target" >&2
  exit 1
fi
assert_file_not_exists "$TARGET1"
assert_eq "1" "$(find "$REPO_FIXTURE/docs/images" -maxdepth 1 -type f -name 'agent-sync-screenshot-*.png' | wc -l | tr -d ' ')" "old managed hash file should be cleaned up"

HOME_NO_PNG="$TMP_ROOT/home-no-png"
REPO_NO_PNG="$TMP_ROOT/repo-no-png"
mkdir -p "$HOME_NO_PNG/Screenshots" "$REPO_NO_PNG/docs/images"
cat >"$REPO_NO_PNG/README.md" <<'EOF'
# SkillsSync
![SkillsSync screenshot](docs/images/agent-sync-latest-screenshot.png?v=20260221-123317)
EOF

if HOME="$HOME_NO_PNG" "$SCRIPT" "$REPO_NO_PNG" >"$TMP_ROOT/out-no-png.txt" 2>"$TMP_ROOT/err-no-png.txt"; then
  echo "ASSERT FAILED: expected non-zero exit when no PNG exists" >&2
  exit 1
fi
assert_contains "no png images found in" "$TMP_ROOT/err-no-png.txt"

HOME_NO_README_MATCH="$TMP_ROOT/home-no-readme-match"
REPO_NO_README_MATCH="$TMP_ROOT/repo-no-readme-match"
mkdir -p "$HOME_NO_README_MATCH/Screenshots" "$REPO_NO_README_MATCH/docs/images"
cat >"$REPO_NO_README_MATCH/README.md" <<'EOF'
# SkillsSync
No screenshot reference here.
EOF
decode_b64 "$HOME_NO_README_MATCH/Screenshots/only.png" \
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAAAAAA6fptVAAAACklEQVR4nGNgAAAAAgABSK+kcQAAAABJRU5ErkJggg=="
touch -t 202602210201 "$HOME_NO_README_MATCH/Screenshots/only.png"

OUT5="$TMP_ROOT/out5.txt"
HOME="$HOME_NO_README_MATCH" "$SCRIPT" "$REPO_NO_README_MATCH" >"$OUT5"
assert_eq "false" "$(get_kv "$OUT5" "readme_updated")" "readme_updated should be false when no screenshot reference exists"
assert_file_exists "$(get_kv "$OUT5" "target")"

echo "All update_screenshot.sh tests passed."
