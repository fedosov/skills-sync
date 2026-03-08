#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MultilineStringState {
    Basic,
    Literal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct McpServerHeader {
    key: String,
    is_subtable: bool,
}

pub(crate) fn toml_line_outside_multiline_flags(content: &str) -> Vec<bool> {
    let mut state = None;
    let mut flags = Vec::new();

    for line in content.lines() {
        flags.push(state.is_none());
        state = multiline_state_after_line(line, state);
    }

    flags
}

pub(crate) fn scan_toml_mcp_server_keys(content: &str) -> Vec<String> {
    let outside_flags = toml_line_outside_multiline_flags(content);
    content
        .lines()
        .zip(outside_flags)
        .filter_map(|(line, outside_multiline)| {
            outside_multiline
                .then(|| extract_mcp_server_key(line))
                .flatten()
        })
        .collect()
}

pub(crate) fn extract_mcp_server_key(line: &str) -> Option<String> {
    let header = parse_mcp_server_header(line)?;
    (!header.is_subtable).then_some(header.key)
}

pub(crate) fn line_matches_mcp_server_subtable(line: &str, key: &str) -> bool {
    parse_mcp_server_header(line).is_some_and(|header| header.is_subtable && header.key == key)
}

fn parse_mcp_server_header(line: &str) -> Option<McpServerHeader> {
    let sanitized = strip_toml_inline_comment(line);
    let trimmed = sanitized.trim();
    let inner = trimmed.strip_prefix('[')?.strip_suffix(']')?;
    let probe_key = "__agent_sync_probe__";
    let probe = format!("[{inner}]\n{probe_key} = true\n");
    let parsed: toml::Table = toml::from_str(&probe).ok()?;
    let root = parsed.get("mcp_servers")?.as_table()?;
    if root.len() != 1 {
        return None;
    }
    let (key, value) = root.iter().next()?;
    let server_table = value.as_table()?;
    value_contains_probe_key(value, probe_key).then(|| McpServerHeader {
        key: key.clone(),
        is_subtable: !server_table.contains_key(probe_key),
    })
}

fn value_contains_probe_key(value: &toml::Value, probe_key: &str) -> bool {
    match value {
        toml::Value::Table(table) => {
            table.contains_key(probe_key)
                || table
                    .values()
                    .any(|nested_value| value_contains_probe_key(nested_value, probe_key))
        }
        _ => false,
    }
}

fn multiline_state_after_line(
    line: &str,
    initial_state: Option<MultilineStringState>,
) -> Option<MultilineStringState> {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    let mut state = initial_state;

    while index < bytes.len() {
        match state {
            None => match bytes[index] {
                b'#' => break,
                b'"' => {
                    if bytes[index..].starts_with(b"\"\"\"") {
                        index += 3;
                        state = Some(MultilineStringState::Basic);
                    } else {
                        index = skip_basic_string(bytes, index + 1);
                    }
                }
                b'\'' => {
                    if bytes[index..].starts_with(b"'''") {
                        index += 3;
                        state = Some(MultilineStringState::Literal);
                    } else {
                        index = skip_literal_string(bytes, index + 1);
                    }
                }
                _ => {
                    index += 1;
                }
            },
            Some(MultilineStringState::Basic) => {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                } else if bytes[index..].starts_with(b"\"\"\"") {
                    index += 3;
                    state = None;
                } else {
                    index += 1;
                }
            }
            Some(MultilineStringState::Literal) => {
                if bytes[index..].starts_with(b"'''") {
                    index += 3;
                    state = None;
                } else {
                    index += 1;
                }
            }
        }
    }

    state
}

fn skip_basic_string(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index = (index + 2).min(bytes.len()),
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    index
}

fn skip_literal_string(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() {
        if bytes[index] == b'\'' {
            return index + 1;
        }
        index += 1;
    }
    index
}

fn strip_toml_inline_comment(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    let mut in_basic_string = false;
    let mut in_literal_string = false;

    while index < bytes.len() {
        if in_basic_string {
            match bytes[index] {
                b'\\' => index = (index + 2).min(bytes.len()),
                b'"' => {
                    in_basic_string = false;
                    index += 1;
                }
                _ => index += 1,
            }
            continue;
        }

        if in_literal_string {
            if bytes[index] == b'\'' {
                in_literal_string = false;
            }
            index += 1;
            continue;
        }

        match bytes[index] {
            b'#' => return line[..index].trim_end().to_string(),
            b'"' => {
                in_basic_string = true;
                index += 1;
            }
            b'\'' => {
                in_literal_string = true;
                index += 1;
            }
            _ => index += 1,
        }
    }

    line.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        extract_mcp_server_key, line_matches_mcp_server_subtable, scan_toml_mcp_server_keys,
    };

    #[test]
    fn scan_toml_mcp_server_keys_ignores_multiline_string_content() {
        let content = r#"
notes = """
[mcp_servers.fake]
"""

[mcp_servers.real]
command = "npx"
"#;

        assert_eq!(scan_toml_mcp_server_keys(content), vec!["real"]);
    }

    #[test]
    fn extract_mcp_server_key_supports_inline_comments() {
        assert_eq!(
            extract_mcp_server_key(r#"[mcp_servers."exa#prod"] # keep"#),
            Some(String::from("exa#prod"))
        );
    }

    #[test]
    fn extract_mcp_server_key_supports_legal_header_whitespace() {
        assert_eq!(
            extract_mcp_server_key("[ mcp_servers.exa ]"),
            Some(String::from("exa"))
        );
        assert_eq!(
            extract_mcp_server_key("[mcp_servers . exa]"),
            Some(String::from("exa"))
        );
        assert_eq!(
            extract_mcp_server_key(r#"[ mcp_servers . "exa prod" ]"#),
            Some(String::from("exa prod"))
        );
    }

    #[test]
    fn extract_mcp_server_key_rejects_subtables_with_legal_header_whitespace() {
        assert_eq!(extract_mcp_server_key("[ mcp_servers.exa.env ]"), None);
        assert_eq!(extract_mcp_server_key("[mcp_servers . exa . env]"), None);
    }

    #[test]
    fn line_matches_mcp_server_subtable_supports_legal_header_whitespace() {
        assert!(line_matches_mcp_server_subtable(
            "[ mcp_servers.exa . env ]",
            "exa"
        ));
        assert!(line_matches_mcp_server_subtable(
            r#"[ mcp_servers . "exa prod" . env ]"#,
            "exa prod"
        ));
        assert!(!line_matches_mcp_server_subtable(
            "[mcp_servers.exa]",
            "exa"
        ));
    }
}
