#!/usr/bin/env bash
# Architecture guard: file size limits + contract sync check.
# Run from repo root: ./scripts/check-architecture.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
EXIT_CODE=0

# --- File size guard (warn on files > 500 lines in core crate) ---
echo "=== File size check (core crate, max 500 lines) ==="
CORE_SRC="$ROOT_DIR/platform/crates/agent-sync-core/src"
SIZE_WARNINGS=0
while IFS= read -r -d '' file; do
    lines=$(wc -l < "$file")
    if [ "$lines" -gt 500 ]; then
        echo "WARNING: $(basename "$file") has $lines lines (limit: 500)"
        echo "  -> Consider splitting into smaller modules for agent legibility."
        SIZE_WARNINGS=$((SIZE_WARNINGS + 1))
    fi
done < <(find "$CORE_SRC" -name '*.rs' -print0)

if [ "$SIZE_WARNINGS" -gt 0 ]; then
    echo "  $SIZE_WARNINGS file(s) exceed size limit (non-blocking)."
else
    echo "  All core files are within size limits."
fi

# --- Contract sync: check that key types exist in both Rust and TS ---
echo ""
echo "=== Contract sync check (Rust models <-> TS types) ==="
RUST_MODELS="$ROOT_DIR/platform/crates/agent-sync-core/src/models.rs"
TS_TYPES="$ROOT_DIR/platform/apps/agent-sync-desktop/ui/src/types.ts"

# Key struct/enum names that must appear in both files
SHARED_TYPES=(
    "SyncState"
    "SyncSummary"
    "SkillRecord"
    "SubagentRecord"
    "McpServerRecord"
    "SyncHealthStatus"
    "SkillLifecycleStatus"
    "AuditEvent"
)

for type_name in "${SHARED_TYPES[@]}"; do
    in_rust=$(grep -c "$type_name" "$RUST_MODELS" 2>/dev/null || true)
    in_ts=$(grep -c "$type_name" "$TS_TYPES" 2>/dev/null || true)
    if [ "$in_rust" -eq 0 ]; then
        echo "DRIFT: $type_name missing from Rust models"
        EXIT_CODE=1
    elif [ "$in_ts" -eq 0 ]; then
        echo "DRIFT: $type_name missing from TS types"
        EXIT_CODE=1
    fi
done

if [ "$EXIT_CODE" -eq 0 ]; then
    echo "  All shared types present in both Rust and TS."
fi

# --- Tauri command locking guard ---
echo ""
echo "=== Tauri command locking guard ==="
TAURI_COMMANDS="$ROOT_DIR/platform/apps/agent-sync-desktop/src-tauri/src/commands"
if grep -R -nE 'acquire_sync_lock|sync_lock\.lock' "$TAURI_COMMANDS"/*.rs >/dev/null 2>&1; then
    echo "DRIFT: Tauri commands must route locking through AppRuntime/command_support helpers"
    grep -R -nE 'acquire_sync_lock|sync_lock\.lock' "$TAURI_COMMANDS"/*.rs || true
    EXIT_CODE=1
else
    echo "  Tauri commands route locking through AppRuntime helpers."
fi

# --- State schema version check ---
echo ""
echo "=== State schema version check ==="
SCHEMA="$ROOT_DIR/platform/spec/state.schema.json"
if [ -f "$SCHEMA" ]; then
    schema_version=$(grep -o '"version"' "$SCHEMA" | head -1)
    if [ -z "$schema_version" ]; then
        echo "WARNING: state.schema.json has no version field"
    else
        echo "  state.schema.json has version field."
    fi
else
    echo "WARNING: state.schema.json not found"
    EXIT_CODE=1
fi

# --- Design decision docs check ---
echo ""
echo "=== Design decision docs check ==="
DECISION_INDEX="$ROOT_DIR/docs/design-docs/index.md"
indexed_decisions=()
while IFS= read -r decision; do
    indexed_decisions+=("$decision")
done < <(
    grep '^| DD-' "$DECISION_INDEX" 2>/dev/null |
        sed -En 's/.*\((DD-[^)]+\.md)\).*/\1/p' |
        sort -u
)

documented_decisions=()
while IFS= read -r decision; do
    documented_decisions+=("$decision")
done < <(
    find "$ROOT_DIR/docs/design-docs" -maxdepth 1 -type f -name 'DD-*.md' -exec basename {} \; |
        sort -u
)

if [ "${#indexed_decisions[@]}" -eq 0 ]; then
    echo "WARNING: design decision index has no active DD entries"
    EXIT_CODE=1
else
    missing_files=$(comm -23 \
        <(printf '%s\n' "${indexed_decisions[@]}") \
        <(printf '%s\n' "${documented_decisions[@]}"))
    missing_index_entries=$(comm -13 \
        <(printf '%s\n' "${indexed_decisions[@]}") \
        <(printf '%s\n' "${documented_decisions[@]}"))

    if [ -n "$missing_files" ] || [ -n "$missing_index_entries" ]; then
        echo "DRIFT: design decision index/file set mismatch"
        if [ -n "$missing_files" ]; then
            echo "  Indexed but missing file(s):"
            printf '%s\n' "$missing_files" | sed 's/^/    - /'
        fi
        if [ -n "$missing_index_entries" ]; then
            echo "  File(s) missing from index:"
            printf '%s\n' "$missing_index_entries" | sed 's/^/    - /'
        fi
        EXIT_CODE=1
    else
        echo "  ${#documented_decisions[@]} DD files match indexed decisions."
    fi
fi

echo ""
if [ "$EXIT_CODE" -ne 0 ]; then
    echo "Architecture checks found issues. See warnings above."
else
    echo "All architecture checks passed."
fi

exit $EXIT_CODE
