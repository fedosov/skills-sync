//! Structural tests that enforce layer dependency direction.
//!
//! Layer ordering (top to bottom):
//!   Types & Models  (models.rs, error.rs)
//!   Paths & Config  (paths.rs, settings.rs, managed_block.rs)
//!   Registries      (codex_registry, codex_subagent_registry, mcp_registry)
//!   Adapters        (dotagents_adapter, dotagents_runtime, agents_context)
//!   Engine          (engine.rs)
//!   Persistence     (state_store, audit_store)
//!   Watch           (watch.rs)

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

/// Parse `use crate::` imports from a source file and return the imported module names.
fn crate_imports(src_path: &Path) -> BTreeSet<String> {
    let content = fs::read_to_string(src_path).unwrap_or_default();
    let mut imports = BTreeSet::new();
    let mut statement = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if statement.is_empty() {
            if !trimmed.starts_with("use crate::") {
                continue;
            }
            statement.push_str(trimmed);
        } else {
            statement.push(' ');
            statement.push_str(trimmed);
        }

        if trimmed.ends_with(';') {
            imports.extend(parse_crate_use_statement(&statement));
            statement.clear();
        }
    }

    imports
}

fn parse_crate_use_statement(statement: &str) -> BTreeSet<String> {
    let Some(after_crate) = statement
        .trim()
        .strip_prefix("use crate::")
        .map(|value| value.trim_end_matches(';').trim())
    else {
        return BTreeSet::new();
    };

    if let Some(grouped) = after_crate.strip_prefix('{') {
        let inner = grouped.strip_suffix('}').unwrap_or(grouped);
        return split_top_level_imports(inner)
            .into_iter()
            .filter_map(|segment| top_level_module_name(&segment))
            .collect();
    }

    top_level_module_name(after_crate).into_iter().collect()
}

fn split_top_level_imports(grouped: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;

    for ch in grouped.chars() {
        match ch {
            '{' => {
                depth += 1;
                current.push(ch);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }

    parts
}

fn top_level_module_name(segment: &str) -> Option<String> {
    let module = segment
        .split("::")
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(|ch| matches!(ch, '{' | '}' | ','));

    (!module.is_empty()).then(|| module.to_string())
}

fn core_src_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn expected_dependency_matrix() -> HashMap<&'static str, BTreeSet<&'static str>> {
    HashMap::from([
        ("agents_context.rs", BTreeSet::from(["models"])),
        (
            "audit_store.rs",
            BTreeSet::from(["error", "models", "paths"]),
        ),
        (
            "codex_registry.rs",
            BTreeSet::from(["managed_block", "models"]),
        ),
        (
            "codex_subagent_registry.rs",
            BTreeSet::from(["managed_block"]),
        ),
        (
            "config_validation.rs",
            BTreeSet::from(["models", "toml_scan"]),
        ),
        (
            "dotagents_adapter.rs",
            BTreeSet::from(["dotagents_runtime", "error"]),
        ),
        ("dotagents_runtime.rs", BTreeSet::from(["error"])),
        (
            "engine.rs",
            BTreeSet::from([
                "agents_context",
                "audit_store",
                "codex_registry",
                "codex_subagent_registry",
                "dotagents_adapter",
                "dotagents_runtime",
                "error",
                "mcp_registry",
                "models",
                "paths",
                "settings",
                "state_store",
                "watch",
            ]),
        ),
        ("error.rs", BTreeSet::from(["models"])),
        ("managed_block.rs", BTreeSet::new()),
        (
            "mcp_registry.rs",
            BTreeSet::from(["error", "managed_block", "models", "toml_scan"]),
        ),
        ("models.rs", BTreeSet::new()),
        ("paths.rs", BTreeSet::new()),
        ("settings.rs", BTreeSet::from(["error", "paths"])),
        (
            "state_store.rs",
            BTreeSet::from(["error", "models", "paths"]),
        ),
        ("toml_scan.rs", BTreeSet::new()),
        ("watch.rs", BTreeSet::new()),
    ])
}

#[test]
fn all_core_modules_are_accounted_for_in_dependency_matrix() {
    let src = core_src_dir();
    let expected = expected_dependency_matrix();
    let actual: BTreeSet<_> = fs::read_dir(&src)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
        .filter(|entry| entry.file_name() != "lib.rs")
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();

    let expected_files: BTreeSet<_> = expected.keys().map(|name| (*name).to_string()).collect();
    assert_eq!(
        actual, expected_files,
        "Dependency matrix is missing core modules or references files that no longer exist."
    );
}

#[test]
fn modules_only_import_allowed_dependencies() {
    let src = core_src_dir();
    let expected = expected_dependency_matrix();

    for (file, allowed) in &expected {
        let imports = crate_imports(&src.join(file));
        let unexpected: Vec<_> = imports
            .iter()
            .filter(|module| !allowed.contains(module.as_str()))
            .cloned()
            .collect();
        assert!(
            unexpected.is_empty(),
            "{file} imports disallowed modules: {:?}. Allowed: {:?}",
            unexpected,
            allowed
        );
    }
}

#[test]
fn models_and_leaf_modules_have_no_upward_imports() {
    let src = core_src_dir();

    for file in [
        "models.rs",
        "paths.rs",
        "managed_block.rs",
        "watch.rs",
        "toml_scan.rs",
    ] {
        let imports = crate_imports(&src.join(file));
        assert!(
            imports.is_empty(),
            "{file} should not depend on sibling modules but found: {:?}",
            imports
        );
    }
}

#[test]
fn dependency_summary() {
    let src = core_src_dir();
    let expected = expected_dependency_matrix();

    for file in expected.keys() {
        let imports = crate_imports(&src.join(file));
        if !imports.is_empty() {
            eprintln!(
                "  {file} -> {}",
                imports.into_iter().collect::<Vec<_>>().join(", ")
            );
        }
    }
}

#[test]
fn crate_imports_tracks_multiline_grouped_use_statements() {
    let tempdir = tempdir().expect("create temp dir");
    let file = tempdir.path().join("sample.rs");
    fs::write(
        &file,
        "use crate::{\n    models,\n    paths::{self, PathKind},\n    error as sync_error,\n};\n",
    )
    .expect("write sample source");

    let imports = crate_imports(&file);

    assert_eq!(
        imports,
        BTreeSet::from([
            String::from("error"),
            String::from("models"),
            String::from("paths"),
        ])
    );
}
