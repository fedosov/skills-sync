use crate::models::{ConfigFormat, ConfigValidationResult};
use crate::toml_scan::scan_toml_mcp_server_keys;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Validate Codex `~/.codex/config.toml` for syntax and duplicate MCP keys.
pub fn validate_codex_config(home: &Path) -> Option<ConfigValidationResult> {
    validate_codex_config_path(home.join(".codex").join("config.toml"))
}

fn validate_codex_config_path(path: PathBuf) -> Option<ConfigValidationResult> {
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    let mut result = ConfigValidationResult {
        path,
        format: ConfigFormat::Toml,
        valid_syntax: true,
        syntax_error: None,
        duplicate_keys: Vec::new(),
        warnings: Vec::new(),
    };

    // Check TOML syntax
    if let Err(e) = toml::from_str::<toml::Table>(&content) {
        result.valid_syntax = false;
        result.syntax_error = Some(e.to_string());
    }

    // Scan for duplicate [mcp_servers.*] headers (text-based, no parsing needed)
    let mut key_counts: HashMap<String, usize> = HashMap::new();
    for key in scan_toml_mcp_server_keys(&content) {
        *key_counts.entry(key).or_insert(0) += 1;
    }
    for (key, count) in &key_counts {
        if *count > 1 {
            result.duplicate_keys.push(key.clone());
        }
    }
    result.duplicate_keys.sort();

    Some(result)
}

/// Validate Claude `~/.claude.json` for syntax and duplicate MCP keys.
pub fn validate_claude_config(home: &Path) -> Option<ConfigValidationResult> {
    validate_json_config_path(home.join(".claude.json"))
}

/// Validate all supported Claude JSON config files.
pub fn validate_claude_configs(home: &Path) -> Vec<ConfigValidationResult> {
    [
        home.join(".claude.json"),
        home.join(".claude").join("settings.local.json"),
        home.join(".claude").join("settings.json"),
    ]
    .into_iter()
    .filter_map(validate_json_config_path)
    .collect()
}

fn validate_json_config_path(path: PathBuf) -> Option<ConfigValidationResult> {
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    let mut result = ConfigValidationResult {
        path,
        format: ConfigFormat::Json,
        valid_syntax: true,
        syntax_error: None,
        duplicate_keys: Vec::new(),
        warnings: Vec::new(),
    };

    // Check JSON syntax
    match scan_json_duplicate_mcp_keys(&content) {
        Ok(duplicate_keys) => {
            result.duplicate_keys = duplicate_keys;
        }
        Err(e) => {
            result.valid_syntax = false;
            result.syntax_error = Some(e.to_string());
        }
    }

    Some(result)
}

/// Validate all known config files (Codex TOML + Claude JSON).
pub fn validate_all_configs(home: &Path, workspaces: &[PathBuf]) -> Vec<ConfigValidationResult> {
    let mut results = Vec::new();
    if let Some(r) = validate_codex_config(home) {
        results.push(r);
    }
    results.extend(validate_claude_configs(home));
    for workspace in workspaces {
        if let Some(result) =
            validate_codex_config_path(workspace.join(".codex").join("config.toml"))
        {
            results.push(result);
        }
        if let Some(result) = validate_json_config_path(workspace.join(".mcp.json")) {
            results.push(result);
        }
    }
    results
}

/// Scan raw JSON text for duplicate server keys inside each `mcpServers` object.
///
/// JSON parsers silently deduplicate object keys, so we walk the stream with a
/// custom visitor and track duplicates per `mcpServers` map rather than across
/// the whole document.
fn scan_json_duplicate_mcp_keys(content: &str) -> Result<Vec<String>, serde_json::Error> {
    let mut duplicates = BTreeSet::new();
    let mut deserializer = serde_json::Deserializer::from_str(content);
    JsonValueSeed {
        duplicates: &mut duplicates,
    }
    .deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(duplicates.into_iter().collect())
}

struct JsonValueSeed<'a> {
    duplicates: &'a mut BTreeSet<String>,
}

impl<'de> DeserializeSeed<'de> for JsonValueSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(JsonValueVisitor {
            duplicates: self.duplicates,
        })
    }
}

struct JsonValueVisitor<'a> {
    duplicates: &'a mut BTreeSet<String>,
}

impl<'de> Visitor<'de> for JsonValueVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any JSON value")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        JsonValueSeed {
            duplicates: self.duplicates,
        }
        .deserialize(deserializer)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while let Some(()) = seq.next_element_seed(JsonValueSeed {
            duplicates: self.duplicates,
        })? {}
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while let Some(key) = map.next_key::<String>()? {
            if key == "mcpServers" {
                map.next_value_seed(McpServersSeed {
                    duplicates: self.duplicates,
                })?;
            } else {
                map.next_value_seed(JsonValueSeed {
                    duplicates: self.duplicates,
                })?;
            }
        }
        Ok(())
    }
}

struct McpServersSeed<'a> {
    duplicates: &'a mut BTreeSet<String>,
}

impl<'de> DeserializeSeed<'de> for McpServersSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(McpServersVisitor {
            duplicates: self.duplicates,
        })
    }
}

struct McpServersVisitor<'a> {
    duplicates: &'a mut BTreeSet<String>,
}

impl<'de> Visitor<'de> for McpServersVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object containing MCP server definitions")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                self.duplicates.insert(key);
            }
            map.next_value::<IgnoredAny>()?;
        }
        Ok(())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while let Some(()) = seq.next_element_seed(JsonValueSeed {
            duplicates: self.duplicates,
        })? {}
        Ok(())
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        JsonValueSeed {
            duplicates: self.duplicates,
        }
        .deserialize(deserializer)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn validate_codex_valid_toml() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let codex_dir = home.join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(
            codex_dir.join("config.toml"),
            r#"[mcp_servers.exa]
type = "sse"
url = "http://localhost:1234"
"#,
        )
        .unwrap();

        let result = validate_codex_config(home).unwrap();
        assert!(result.valid_syntax);
        assert!(result.duplicate_keys.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn validate_codex_detects_duplicate_keys() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let codex_dir = home.join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(
            codex_dir.join("config.toml"),
            r#"[mcp_servers.exa]
type = "sse"
url = "http://localhost:1234"

[mcp_servers.exa]
type = "sse"
url = "http://localhost:5678"
"#,
        )
        .unwrap();

        let result = validate_codex_config(home).unwrap();
        assert!(!result.valid_syntax, "duplicate keys make TOML invalid");
        assert_eq!(result.duplicate_keys, vec!["exa"]);
    }

    #[test]
    fn validate_codex_missing_file_returns_none() {
        let tmp = TempDir::new().unwrap();
        assert!(validate_codex_config(tmp.path()).is_none());
    }

    #[test]
    fn validate_codex_ignores_multiline_string_header_text() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let codex_dir = home.join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(
            codex_dir.join("config.toml"),
            r#"notes = """
[mcp_servers.exa]
"""

[mcp_servers.exa_real]
type = "sse"
url = "http://localhost:1234"
"#,
        )
        .unwrap();

        let result = validate_codex_config(home).unwrap();
        assert!(
            result.valid_syntax,
            "multiline string content is valid TOML"
        );
        assert!(
            result.duplicate_keys.is_empty(),
            "header-like text inside multiline strings should not be reported"
        );
    }

    #[test]
    fn validate_claude_valid_json() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        fs::write(
            home.join(".claude.json"),
            r#"{
  "mcpServers": {
    "exa": { "command": "npx", "args": ["-y", "exa-mcp-server"] }
  }
}"#,
        )
        .unwrap();

        let result = validate_claude_config(home).unwrap();
        assert!(result.valid_syntax);
        assert!(result.duplicate_keys.is_empty());
        assert_eq!(result.path, home.join(".claude.json"));
    }

    #[test]
    fn validate_claude_configs_includes_all_supported_paths() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let claude_dir = home.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(
            home.join(".claude.json"),
            r#"{
  "mcpServers": {
    "exa": { "command": "npx", "args": ["-y", "exa-mcp-server"] }
  }
}"#,
        )
        .unwrap();
        fs::write(
            claude_dir.join("settings.local.json"),
            r#"{"mcpServers": {}}"#,
        )
        .unwrap();
        fs::write(claude_dir.join("settings.json"), r#"{"mcpServers": {}}"#).unwrap();

        let results = validate_claude_configs(home);
        let paths = results
            .into_iter()
            .map(|result| result.path)
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                home.join(".claude.json"),
                claude_dir.join("settings.local.json"),
                claude_dir.join("settings.json"),
            ]
        );
    }

    #[test]
    fn validate_claude_detects_syntax_error() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        fs::write(home.join(".claude.json"), r#"{ "mcpServers": { broken }"#).unwrap();

        let result = validate_claude_config(home).unwrap();
        assert!(!result.valid_syntax);
        assert!(result.syntax_error.is_some());
    }

    #[test]
    fn validate_claude_rejects_trailing_garbage() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        fs::write(
            home.join(".claude.json"),
            r#"{"mcpServers": {}} trailing-garbage"#,
        )
        .unwrap();

        let result = validate_claude_config(home).unwrap();
        assert!(!result.valid_syntax);
        assert!(result.syntax_error.is_some());
    }

    #[test]
    fn validate_claude_duplicate_keys_are_scoped_per_mcp_servers_object() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        fs::write(
            home.join(".claude.json"),
            r#"{
  "mcpServers": {
    "exa": { "command": "global-exa" }
  },
  "projects": {
    "/tmp/workspace-a": {
      "mcpServers": {
        "exa": { "command": "project-exa" }
      }
    }
  }
}"#,
        )
        .unwrap();

        let result = validate_claude_config(home).unwrap();
        assert!(result.valid_syntax);
        assert!(
            result.duplicate_keys.is_empty(),
            "same key in different mcpServers objects should not warn"
        );
    }

    #[test]
    fn validate_claude_detects_project_only_duplicate_keys() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        fs::write(
            home.join(".claude.json"),
            r#"{
  "projects": {
    "/tmp/workspace-a": {
      "mcpServers": {
        "exa": { "command": "first" },
        "exa": { "command": "second" }
      }
    }
  }
}"#,
        )
        .unwrap();

        let result = validate_claude_config(home).unwrap();
        assert!(result.valid_syntax);
        assert_eq!(result.duplicate_keys, vec!["exa"]);
    }

    #[test]
    fn validate_all_returns_codex_and_supported_claude_configs() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::write(
            home.join(".codex").join("config.toml"),
            "[mcp_servers.foo]\ntype = \"sse\"\n",
        )
        .unwrap();
        fs::write(home.join(".claude.json"), r#"{"mcpServers": {}}"#).unwrap();
        fs::write(
            home.join(".claude").join("settings.local.json"),
            r#"{"mcpServers": {}}"#,
        )
        .unwrap();

        let results = validate_all_configs(home, &[]);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].format, ConfigFormat::Toml);
        assert_eq!(results[1].format, ConfigFormat::Json);
        assert_eq!(results[2].format, ConfigFormat::Json);
    }

    #[test]
    fn validate_all_includes_workspace_codex_and_mcp_configs() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let workspace = home.join("workspace-a");
        fs::create_dir_all(workspace.join(".codex")).unwrap();
        fs::write(
            workspace.join(".codex").join("config.toml"),
            "[mcp_servers.foo]\ntype = \"sse\"\nurl = \"http://localhost:1234\"\n\n[mcp_servers.foo]\ntype = \"sse\"\nurl = \"http://localhost:5678\"\n",
        )
        .unwrap();
        fs::write(
            workspace.join(".mcp.json"),
            r#"{"mcpServers":{"exa":{"command":"first"},"exa":{"command":"second"}}}"#,
        )
        .unwrap();

        let results = validate_all_configs(home, std::slice::from_ref(&workspace));
        assert_eq!(results.len(), 2);

        let workspace_codex = results
            .iter()
            .find(|result| result.path == workspace.join(".codex").join("config.toml"))
            .expect("workspace codex validation");
        assert_eq!(workspace_codex.format, ConfigFormat::Toml);
        assert!(!workspace_codex.valid_syntax);
        assert_eq!(workspace_codex.duplicate_keys, vec!["foo"]);

        let workspace_mcp = results
            .iter()
            .find(|result| result.path == workspace.join(".mcp.json"))
            .expect("workspace mcp validation");
        assert_eq!(workspace_mcp.format, ConfigFormat::Json);
        assert!(workspace_mcp.valid_syntax);
        assert_eq!(workspace_mcp.duplicate_keys, vec!["exa"]);
    }
}
