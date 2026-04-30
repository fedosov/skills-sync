use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

const FRONTMATTER_READ_CAP_BYTES: u64 = 8 * 1024;

pub fn combine_output(primary: &str, secondary: &str) -> String {
    match (primary.trim().is_empty(), secondary.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => primary.trim().to_string(),
        (true, false) => secondary.trim().to_string(),
        (false, false) => format!("{}\n{}", primary.trim(), secondary.trim()),
    }
}

pub fn require_non_empty(value: &str, message: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(message.to_string());
    }
    Ok(())
}

pub fn read_skill_description(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file).take(FRONTMATTER_READ_CAP_BYTES);
    let mut header = String::new();
    reader.read_to_string(&mut header).ok()?;

    let trimmed = header.trim_start_matches('\u{feff}').trim_start();
    let after_open = trimmed
        .strip_prefix("---")?
        .trim_start_matches(['\n', '\r']);
    let close_pos = after_open.find("\n---")?;
    let frontmatter = &after_open[..close_pos];

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("description:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{combine_output, read_skill_description, require_non_empty};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn combine_output_handles_all_combinations() {
        assert_eq!(combine_output("", ""), "");
        assert_eq!(combine_output("a", ""), "a");
        assert_eq!(combine_output("", "b"), "b");
        assert_eq!(combine_output("a", "b"), "a\nb");
        assert_eq!(combine_output(" a ", " "), "a");
    }

    #[test]
    fn require_non_empty_rejects_blank() {
        assert!(require_non_empty("   ", "blank").is_err());
        assert!(require_non_empty("ok", "blank").is_ok());
    }

    #[test]
    fn read_skill_description_parses_frontmatter() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("SKILL.md");
        fs::write(
            &path,
            "---\nname: foo\ndescription: A short skill\n---\n\nbody",
        )
        .expect("write");
        assert_eq!(
            read_skill_description(&path).as_deref(),
            Some("A short skill")
        );
    }

    #[test]
    fn read_skill_description_skips_when_no_frontmatter() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("SKILL.md");
        fs::write(&path, "no frontmatter here").expect("write");
        assert!(read_skill_description(&path).is_none());
    }

    #[test]
    fn read_skill_description_strips_bom() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("SKILL.md");
        fs::write(&path, "\u{feff}---\ndescription: X\n---\n").expect("write");
        assert_eq!(read_skill_description(&path).as_deref(), Some("X"));
    }
}
