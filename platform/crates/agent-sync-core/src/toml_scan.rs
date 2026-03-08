#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MultilineStringState {
    Basic,
    Literal,
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
    let sanitized = strip_toml_inline_comment(line);
    let trimmed = sanitized.trim();
    let inner = trimmed.strip_prefix('[')?.strip_suffix(']')?;
    let rest = inner.strip_prefix("mcp_servers.")?;
    let key = if rest.starts_with('"') {
        rest.strip_prefix('"')?.strip_suffix('"')?.to_string()
    } else if rest.starts_with('\'') {
        rest.strip_prefix('\'')?.strip_suffix('\'')?.to_string()
    } else {
        if rest.contains('.') {
            return None;
        }
        rest.to_string()
    };
    if key.is_empty() {
        return None;
    }
    Some(key)
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
    use super::{extract_mcp_server_key, scan_toml_mcp_server_keys};

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
}
