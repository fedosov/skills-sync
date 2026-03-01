use crate::models::SyncState;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const INCLUDE_MAX_DEPTH: usize = 5;
const FILE_WARNING_TOKENS: usize = 2_000;
const FILE_CRITICAL_TOKENS: usize = 4_000;
const TOTAL_WARNING_TOKENS: usize = 8_000;
const TOTAL_CRITICAL_TOKENS: usize = 16_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentContextSeverity {
    Ok,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentContextSegment {
    pub path: String,
    pub depth: usize,
    pub chars: usize,
    pub lines: usize,
    pub tokens_estimate: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentContextEntry {
    pub id: String,
    pub scope: String,
    pub workspace: Option<String>,
    pub root_path: String,
    pub exists: bool,
    pub severity: AgentContextSeverity,
    pub raw_chars: usize,
    pub raw_lines: usize,
    pub rendered_chars: usize,
    pub rendered_lines: usize,
    pub tokens_estimate: usize,
    pub include_count: usize,
    pub missing_includes: Vec<String>,
    pub cycles_detected: Vec<String>,
    pub max_depth_reached: bool,
    pub diagnostics: Vec<String>,
    pub segments: Vec<AgentContextSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentsContextLimits {
    pub include_max_depth: usize,
    pub file_warning_tokens: usize,
    pub file_critical_tokens: usize,
    pub total_warning_tokens: usize,
    pub total_critical_tokens: usize,
    pub tokens_formula: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentsContextTotals {
    pub roots_count: usize,
    pub rendered_chars: usize,
    pub rendered_lines: usize,
    pub tokens_estimate: usize,
    pub include_count: usize,
    pub missing_include_count: usize,
    pub cycle_count: usize,
    pub max_depth_reached_count: usize,
    pub severity: AgentContextSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentsContextReport {
    pub generated_at: String,
    pub limits: AgentsContextLimits,
    pub totals: AgentsContextTotals,
    pub warning_count: usize,
    pub critical_count: usize,
    pub entries: Vec<AgentContextEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RootTarget {
    scope: String,
    workspace: Option<String>,
    root_path: PathBuf,
    exists: bool,
}

#[derive(Debug, Default)]
struct EntryCollector {
    rendered_chars: usize,
    rendered_lines: usize,
    include_count: usize,
    missing_includes: Vec<String>,
    cycles_detected: Vec<String>,
    max_depth_reached: bool,
    diagnostics: Vec<String>,
    segments: BTreeMap<String, SegmentAccumulator>,
}

#[derive(Debug, Clone, Default)]
struct SegmentAccumulator {
    depth: usize,
    chars: usize,
    lines: usize,
}

pub fn build_agents_context_report(
    home_directory: &Path,
    state: &SyncState,
) -> AgentsContextReport {
    let roots = collect_root_targets(home_directory, state);
    let mut entries = Vec::with_capacity(roots.len());

    for root in roots {
        entries.push(build_entry(home_directory, &root));
    }

    let total_chars = entries
        .iter()
        .map(|entry| entry.rendered_chars)
        .sum::<usize>();
    let total_lines = entries
        .iter()
        .map(|entry| entry.rendered_lines)
        .sum::<usize>();
    let total_includes = entries
        .iter()
        .map(|entry| entry.include_count)
        .sum::<usize>();
    let total_missing = entries
        .iter()
        .map(|entry| entry.missing_includes.len())
        .sum::<usize>();
    let total_cycles = entries
        .iter()
        .map(|entry| entry.cycles_detected.len())
        .sum::<usize>();
    let total_depth = entries
        .iter()
        .filter(|entry| entry.max_depth_reached)
        .count();
    let total_tokens = estimate_tokens(total_chars);

    let warning_count = entries
        .iter()
        .filter(|entry| entry.severity == AgentContextSeverity::Warning)
        .count();
    let critical_count = entries
        .iter()
        .filter(|entry| entry.severity == AgentContextSeverity::Critical)
        .count();

    AgentsContextReport {
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        limits: AgentsContextLimits {
            include_max_depth: INCLUDE_MAX_DEPTH,
            file_warning_tokens: FILE_WARNING_TOKENS,
            file_critical_tokens: FILE_CRITICAL_TOKENS,
            total_warning_tokens: TOTAL_WARNING_TOKENS,
            total_critical_tokens: TOTAL_CRITICAL_TOKENS,
            tokens_formula: String::from("ceil(rendered_chars / 4)"),
        },
        totals: AgentsContextTotals {
            roots_count: entries.len(),
            rendered_chars: total_chars,
            rendered_lines: total_lines,
            tokens_estimate: total_tokens,
            include_count: total_includes,
            missing_include_count: total_missing,
            cycle_count: total_cycles,
            max_depth_reached_count: total_depth,
            severity: severity_for_tokens(
                total_tokens,
                TOTAL_WARNING_TOKENS,
                TOTAL_CRITICAL_TOKENS,
            ),
        },
        warning_count,
        critical_count,
        entries,
    }
}

pub fn project_workspaces_from_state(state: &SyncState) -> Vec<PathBuf> {
    let mut unique = BTreeMap::new();

    for skill in &state.skills {
        insert_project_workspace(
            &mut unique,
            skill.scope.as_str(),
            skill.workspace.as_deref(),
        );
    }
    for subagent in &state.subagents {
        insert_project_workspace(
            &mut unique,
            subagent.scope.as_str(),
            subagent.workspace.as_deref(),
        );
    }
    for server in &state.mcp_servers {
        insert_project_workspace(
            &mut unique,
            server.scope.as_str(),
            server.workspace.as_deref(),
        );
    }

    let mut paths: Vec<PathBuf> = unique.into_values().collect();
    paths.sort();
    paths
}

fn insert_project_workspace(
    unique: &mut BTreeMap<String, PathBuf>,
    scope: &str,
    workspace_raw: Option<&str>,
) {
    if scope != "project" {
        return;
    }

    let Some(workspace_raw) = workspace_raw else {
        return;
    };
    let workspace = workspace_raw.trim();
    if workspace.is_empty() {
        return;
    }

    let workspace_path = PathBuf::from(workspace);
    let key = canonical_key(&workspace_path);
    unique.entry(key).or_insert(workspace_path);
}

fn collect_root_targets(home_directory: &Path, state: &SyncState) -> Vec<RootTarget> {
    let global_roots = vec![
        home_directory.join("AGENTS.md"),
        home_directory.join("Dev").join("AGENTS.md"),
        home_directory
            .join(".config")
            .join("ai-agents")
            .join("AGENTS.md"),
        home_directory.join(".codex").join("AGENTS.md"),
    ];

    let mut roots = Vec::new();
    let mut seen_existing = HashSet::new();

    for root in global_roots {
        if !root.is_file() {
            continue;
        }
        let key = canonical_key(&root);
        if !seen_existing.insert(key) {
            continue;
        }
        roots.push(RootTarget {
            scope: String::from("global"),
            workspace: None,
            root_path: root,
            exists: true,
        });
    }

    for workspace in project_workspaces_from_state(state) {
        let primary = workspace.join("AGENTS.md");
        let fallback = workspace.join("agents.md");
        let (root_path, exists) = if primary.is_file() {
            (primary, true)
        } else if fallback.is_file() {
            (fallback, true)
        } else {
            (primary, false)
        };

        if exists {
            let key = canonical_key(&root_path);
            if !seen_existing.insert(key) {
                continue;
            }
        }

        roots.push(RootTarget {
            scope: String::from("project"),
            workspace: Some(workspace.display().to_string()),
            root_path,
            exists,
        });
    }

    roots
}

fn build_entry(home_directory: &Path, root: &RootTarget) -> AgentContextEntry {
    let root_path_display = root.root_path.display().to_string();
    let entry_id = format!(
        "{}|{}|{}",
        root.scope,
        root.workspace.as_deref().unwrap_or("global"),
        root_path_display
    );

    if !root.exists {
        let diagnostics = if root.scope == "project" {
            vec![format!(
                "root missing: {} (fallback checked: {})",
                root_path_display,
                root.root_path
                    .parent()
                    .map(|parent| parent.join("agents.md").display().to_string())
                    .unwrap_or_else(|| String::from("agents.md"))
            )]
        } else {
            vec![format!("root missing: {}", root_path_display)]
        };

        return AgentContextEntry {
            id: entry_id,
            scope: root.scope.clone(),
            workspace: root.workspace.clone(),
            root_path: root_path_display,
            exists: false,
            severity: AgentContextSeverity::Ok,
            raw_chars: 0,
            raw_lines: 0,
            rendered_chars: 0,
            rendered_lines: 0,
            tokens_estimate: 0,
            include_count: 0,
            missing_includes: Vec::new(),
            cycles_detected: Vec::new(),
            max_depth_reached: false,
            diagnostics,
            segments: Vec::new(),
        };
    }

    let (raw_chars, raw_lines) = read_file_size_metrics(&root.root_path);
    let mut collector = EntryCollector::default();
    let mut stack = Vec::new();
    render_file_recursive(
        home_directory,
        &root.root_path,
        0,
        &mut stack,
        &mut collector,
    );

    let mut missing_includes = collector.missing_includes;
    missing_includes.sort();
    missing_includes.dedup();

    let mut cycles_detected = collector.cycles_detected;
    cycles_detected.sort();
    cycles_detected.dedup();

    let mut diagnostics = collector.diagnostics;
    diagnostics.sort();
    diagnostics.dedup();

    let mut segments: Vec<AgentContextSegment> = collector
        .segments
        .into_iter()
        .map(|(path, stats)| AgentContextSegment {
            path,
            depth: stats.depth,
            chars: stats.chars,
            lines: stats.lines,
            tokens_estimate: estimate_tokens(stats.chars),
        })
        .collect();
    segments.sort_by(|lhs, rhs| {
        rhs.tokens_estimate
            .cmp(&lhs.tokens_estimate)
            .then_with(|| lhs.path.cmp(&rhs.path))
    });

    let tokens_estimate = estimate_tokens(collector.rendered_chars);

    AgentContextEntry {
        id: entry_id,
        scope: root.scope.clone(),
        workspace: root.workspace.clone(),
        root_path: root_path_display,
        exists: true,
        severity: severity_for_tokens(tokens_estimate, FILE_WARNING_TOKENS, FILE_CRITICAL_TOKENS),
        raw_chars,
        raw_lines,
        rendered_chars: collector.rendered_chars,
        rendered_lines: collector.rendered_lines,
        tokens_estimate,
        include_count: collector.include_count,
        missing_includes,
        cycles_detected,
        max_depth_reached: collector.max_depth_reached,
        diagnostics,
        segments,
    }
}

fn render_file_recursive(
    home_directory: &Path,
    file_path: &Path,
    depth: usize,
    stack: &mut Vec<String>,
    collector: &mut EntryCollector,
) {
    let path_display = file_path.display().to_string();
    let path_key = canonical_key(file_path);

    if stack.iter().any(|item| item == &path_key) {
        collector.cycles_detected.push(format!(
            "cycle detected: {} -> {}",
            stack
                .last()
                .cloned()
                .unwrap_or_else(|| String::from("<root>")),
            path_display
        ));
        collector
            .diagnostics
            .push(format!("cycle skipped: {}", path_display));
        return;
    }

    let Ok(content) = fs::read_to_string(file_path) else {
        collector
            .diagnostics
            .push(format!("failed to read include: {}", path_display));
        return;
    };

    let chars = content.chars().count();
    let lines = line_count(&content);

    collector.rendered_chars += chars;
    collector.rendered_lines += lines;

    collector
        .segments
        .entry(path_display.clone())
        .and_modify(|acc| {
            acc.depth = acc.depth.min(depth);
            acc.chars += chars;
            acc.lines += lines;
        })
        .or_insert(SegmentAccumulator {
            depth,
            chars,
            lines,
        });

    let include_specs = extract_include_specs(&content);
    if include_specs.is_empty() {
        return;
    }

    stack.push(path_key);

    for include in include_specs {
        let resolved = resolve_include_path(home_directory, file_path, &include);

        if !resolved.exists() {
            collector.missing_includes.push(format!(
                "{} -> {}",
                file_path.display(),
                resolved.display()
            ));
            collector.diagnostics.push(format!(
                "missing include from {}: {}",
                file_path.display(),
                include
            ));
            continue;
        }

        let mut include_targets = Vec::new();
        if resolved.is_dir() {
            include_targets = collect_markdown_files(&resolved);
            if include_targets.is_empty() {
                collector.diagnostics.push(format!(
                    "directory include has no .md files: {}",
                    resolved.display()
                ));
            }
        } else if resolved.is_file() {
            include_targets.push(resolved);
        }

        for target in include_targets {
            if depth >= INCLUDE_MAX_DEPTH {
                collector.max_depth_reached = true;
                collector.diagnostics.push(format!(
                    "max include depth reached at {} (depth cap {})",
                    target.display(),
                    INCLUDE_MAX_DEPTH
                ));
                continue;
            }
            collector.include_count += 1;
            render_file_recursive(home_directory, &target, depth + 1, stack, collector);
        }
    }

    stack.pop();
}

fn extract_include_specs(content: &str) -> Vec<String> {
    let mut includes = Vec::new();
    let mut in_fenced_block = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if starts_fence(trimmed) {
            in_fenced_block = !in_fenced_block;
            continue;
        }
        if in_fenced_block {
            continue;
        }
        includes.extend(extract_line_include_specs(line));
    }

    includes
}

fn starts_fence(line: &str) -> bool {
    line.starts_with("```") || line.starts_with("~~~")
}

fn extract_line_include_specs(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut includes = Vec::new();
    let mut in_inline_code = false;
    let mut index = 0;

    while index < chars.len() {
        let current = chars[index];

        if current == '`' {
            in_inline_code = !in_inline_code;
            index += 1;
            continue;
        }

        if in_inline_code || current != '@' {
            index += 1;
            continue;
        }

        let previous = if index > 0 {
            Some(chars[index - 1])
        } else {
            None
        };
        if !is_include_boundary(previous) {
            index += 1;
            continue;
        }

        let Some((spec, consumed)) = parse_include_spec(&chars, index + 1) else {
            index += 1;
            continue;
        };
        if !spec.is_empty() {
            includes.push(spec);
        }
        index = index + consumed + 1;
    }

    includes
}

fn is_include_boundary(previous: Option<char>) -> bool {
    match previous {
        None => true,
        Some(value) => {
            value.is_whitespace() || matches!(value, '(' | '[' | '{' | '"' | '\'' | ':' | ';')
        }
    }
}

fn parse_include_spec(chars: &[char], start: usize) -> Option<(String, usize)> {
    if start >= chars.len() {
        return None;
    }

    let quote = chars[start];
    if quote == '"' || quote == '\'' {
        let mut current = start + 1;
        let mut value = String::new();
        while current < chars.len() {
            if chars[current] == quote {
                return Some((sanitize_include_spec(&value), current + 1 - start));
            }
            value.push(chars[current]);
            current += 1;
        }
        return Some((sanitize_include_spec(&value), chars.len() - start));
    }

    if chars[start].is_whitespace() {
        return None;
    }

    let mut current = start;
    let mut value = String::new();
    while current < chars.len() {
        if chars[current].is_whitespace() {
            break;
        }
        value.push(chars[current]);
        current += 1;
    }

    Some((sanitize_include_spec(&value), current - start))
}

fn sanitize_include_spec(raw: &str) -> String {
    raw.trim_end_matches([',', '.', ';', ':', ')', '!', '?'])
        .trim()
        .to_string()
}

fn resolve_include_path(home_directory: &Path, current_file: &Path, include: &str) -> PathBuf {
    let trimmed = include.trim();
    if trimmed == "~" {
        return home_directory.to_path_buf();
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home_directory.join(rest);
    }

    let include_path = PathBuf::from(trimmed);
    if include_path.is_absolute() {
        return include_path;
    }

    current_file
        .parent()
        .map(|parent| parent.join(include_path.clone()))
        .unwrap_or(include_path)
}

fn collect_markdown_files(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(directory)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let is_markdown = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if is_markdown {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    files
}

fn read_file_size_metrics(path: &Path) -> (usize, usize) {
    match fs::read_to_string(path) {
        Ok(content) => (content.chars().count(), line_count(&content)),
        Err(_) => (0, 0),
    }
}

fn line_count(content: &str) -> usize {
    if content.is_empty() {
        0
    } else {
        content.lines().count()
    }
}

fn canonical_key(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn estimate_tokens(char_count: usize) -> usize {
    if char_count == 0 {
        return 0;
    }
    char_count.div_ceil(4)
}

fn severity_for_tokens(
    tokens: usize,
    warning_threshold: usize,
    critical_threshold: usize,
) -> AgentContextSeverity {
    if tokens >= critical_threshold {
        return AgentContextSeverity::Critical;
    }
    if tokens >= warning_threshold {
        return AgentContextSeverity::Warning;
    }
    AgentContextSeverity::Ok
}

#[cfg(test)]
mod tests {
    use super::{
        build_agents_context_report, estimate_tokens, project_workspaces_from_state,
        AgentContextSeverity,
    };
    use crate::models::{
        McpEnabledByAgent, McpServerRecord, McpTransport, SkillLifecycleStatus, SkillRecord,
        SubagentRecord, SyncHealthStatus, SyncMetadata, SyncState, SyncSummary,
    };
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn empty_state() -> SyncState {
        SyncState {
            version: 2,
            generated_at: String::from("2026-02-23T00:00:00Z"),
            sync: SyncMetadata {
                status: SyncHealthStatus::Ok,
                last_started_at: None,
                last_finished_at: None,
                duration_ms: None,
                error: None,
                warnings: Vec::new(),
            },
            summary: SyncSummary::empty(),
            subagent_summary: SyncSummary::empty(),
            skills: Vec::new(),
            subagents: Vec::new(),
            mcp_servers: Vec::new(),
            top_skills: Vec::new(),
            top_subagents: Vec::new(),
        }
    }

    fn project_skill(workspace: &Path, key: &str) -> SkillRecord {
        SkillRecord {
            id: format!("id-{key}"),
            name: format!("Skill {key}"),
            scope: String::from("project"),
            workspace: Some(workspace.display().to_string()),
            canonical_source_path: workspace
                .join(".claude")
                .join("skills")
                .display()
                .to_string(),
            target_paths: Vec::new(),
            exists: true,
            is_symlink_canonical: false,
            package_type: String::from("dir"),
            skill_key: key.to_string(),
            symlink_target: String::new(),
            source: None,
            commit: None,
            install_status: None,
            wildcard_source: None,
            status: SkillLifecycleStatus::Active,
            archived_at: None,
            archived_bundle_path: None,
            archived_original_scope: None,
            archived_original_workspace: None,
        }
    }

    fn project_subagent(workspace: &Path, key: &str) -> SubagentRecord {
        SubagentRecord {
            id: format!("sub-{key}"),
            name: format!("Subagent {key}"),
            description: String::from("desc"),
            scope: String::from("project"),
            workspace: Some(workspace.display().to_string()),
            canonical_source_path: workspace
                .join(".agents")
                .join("subagents")
                .display()
                .to_string(),
            target_paths: Vec::new(),
            exists: true,
            is_symlink_canonical: false,
            package_type: String::from("file"),
            subagent_key: key.to_string(),
            symlink_target: String::new(),
            model: None,
            tools: Vec::new(),
            codex_tools_ignored: false,
            status: SkillLifecycleStatus::Active,
            archived_at: None,
            archived_bundle_path: None,
            archived_original_scope: None,
            archived_original_workspace: None,
        }
    }

    fn project_mcp(workspace: &Path, key: &str) -> McpServerRecord {
        McpServerRecord {
            server_key: key.to_string(),
            scope: String::from("project"),
            workspace: Some(workspace.display().to_string()),
            transport: McpTransport::Stdio,
            command: Some(String::from("cmd")),
            args: Vec::new(),
            url: None,
            env: Default::default(),
            enabled_by_agent: McpEnabledByAgent::default(),
            targets: Vec::new(),
            warnings: Vec::new(),
            status: SkillLifecycleStatus::Active,
            archived_at: None,
        }
    }

    #[test]
    fn resolves_relative_file_includes() {
        let temp = tempdir().expect("temp dir");
        let home = temp.path().join("home");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(home.join("Dev")).expect("create home dev");
        fs::create_dir_all(&workspace).expect("create workspace");

        fs::write(workspace.join("AGENTS.md"), "@./docs/policy.md\n").expect("write root");
        fs::create_dir_all(workspace.join("docs")).expect("create docs");
        fs::write(workspace.join("docs").join("policy.md"), "policy-body").expect("write include");

        let mut state = empty_state();
        state.skills.push(project_skill(&workspace, "a"));

        let report = build_agents_context_report(&home, &state);
        let entry = report.entries.first().expect("entry");
        assert_eq!(entry.include_count, 1);
        assert_eq!(entry.missing_includes.len(), 0);
        assert_eq!(
            entry.rendered_chars,
            "@./docs/policy.md\n".chars().count() + "policy-body".chars().count()
        );
    }

    #[test]
    fn ignores_includes_in_inline_and_fenced_code() {
        let temp = tempdir().expect("temp dir");
        let home = temp.path().join("home");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(home.join("Dev")).expect("create home dev");
        fs::create_dir_all(workspace.join("docs")).expect("create docs");

        let body = ["`@./docs/inline.md`", "```md", "@./docs/fenced.md", "```"].join("\n");

        fs::write(workspace.join("AGENTS.md"), &body).expect("write root");
        fs::write(workspace.join("docs").join("inline.md"), "inline").expect("write inline");
        fs::write(workspace.join("docs").join("fenced.md"), "fenced").expect("write fenced");

        let mut state = empty_state();
        state.skills.push(project_skill(&workspace, "a"));

        let report = build_agents_context_report(&home, &state);
        let entry = report.entries.first().expect("entry");
        assert_eq!(entry.include_count, 0);
        assert_eq!(entry.rendered_chars, body.chars().count());
    }

    #[test]
    fn stops_recursive_includes_at_depth_five() {
        let temp = tempdir().expect("temp dir");
        let home = temp.path().join("home");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(home.join("Dev")).expect("create home dev");
        fs::create_dir_all(&workspace).expect("create workspace");

        fs::write(workspace.join("AGENTS.md"), "@./d1.md\n").expect("write root");
        for depth in 1..=7 {
            let include_line = if depth < 7 {
                format!("@./d{}.md\n", depth + 1)
            } else {
                String::from("leaf\n")
            };
            fs::write(workspace.join(format!("d{depth}.md")), include_line).expect("write chain");
        }

        let mut state = empty_state();
        state.skills.push(project_skill(&workspace, "a"));

        let report = build_agents_context_report(&home, &state);
        let entry = report.entries.first().expect("entry");
        assert!(entry.max_depth_reached);
        assert!(entry.include_count < 7);
    }

    #[test]
    fn detects_cycles_without_infinite_recursion() {
        let temp = tempdir().expect("temp dir");
        let home = temp.path().join("home");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(home.join("Dev")).expect("create home dev");
        fs::create_dir_all(&workspace).expect("create workspace");

        fs::write(workspace.join("AGENTS.md"), "@./a.md\n").expect("write root");
        fs::write(workspace.join("a.md"), "@./b.md\n").expect("write a");
        fs::write(workspace.join("b.md"), "@./a.md\n").expect("write b");

        let mut state = empty_state();
        state.skills.push(project_skill(&workspace, "a"));

        let report = build_agents_context_report(&home, &state);
        let entry = report.entries.first().expect("entry");
        assert!(!entry.cycles_detected.is_empty());
    }

    #[test]
    fn expands_directory_include_recursively_and_sorted() {
        let temp = tempdir().expect("temp dir");
        let home = temp.path().join("home");
        let workspace = temp.path().join("workspace");
        let docs = workspace.join("docs");
        fs::create_dir_all(home.join("Dev")).expect("create home dev");
        fs::create_dir_all(docs.join("nested")).expect("create nested docs");

        fs::write(workspace.join("AGENTS.md"), "@./docs\n").expect("write root");
        fs::write(docs.join("z.md"), "z").expect("write z");
        fs::write(docs.join("a.md"), "a").expect("write a");
        fs::write(docs.join("nested").join("b.md"), "b").expect("write b");
        fs::write(docs.join("nested").join("ignore.txt"), "x").expect("write txt");

        let mut state = empty_state();
        state.skills.push(project_skill(&workspace, "a"));

        let report = build_agents_context_report(&home, &state);
        let entry = report.entries.first().expect("entry");
        assert_eq!(entry.include_count, 3);
        let segment_paths: Vec<&str> = entry
            .segments
            .iter()
            .map(|segment| segment.path.as_str())
            .collect();
        assert!(segment_paths.iter().any(|path| path.ends_with("/a.md")));
        assert!(segment_paths.iter().any(|path| path.ends_with("/b.md")));
        assert!(segment_paths.iter().any(|path| path.ends_with("/z.md")));
        assert!(!segment_paths
            .iter()
            .any(|path| path.ends_with("ignore.txt")));
    }

    #[test]
    fn records_missing_includes_without_crashing() {
        let temp = tempdir().expect("temp dir");
        let home = temp.path().join("home");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(home.join("Dev")).expect("create home dev");
        fs::create_dir_all(&workspace).expect("create workspace");

        fs::write(workspace.join("AGENTS.md"), "@./missing.md\n").expect("write root");

        let mut state = empty_state();
        state.skills.push(project_skill(&workspace, "a"));

        let report = build_agents_context_report(&home, &state);
        let entry = report.entries.first().expect("entry");
        assert_eq!(entry.missing_includes.len(), 1);
        assert!(entry
            .diagnostics
            .iter()
            .any(|item| item.contains("missing include")));
    }

    #[test]
    fn token_estimate_and_severity_thresholds_work() {
        let temp = tempdir().expect("temp dir");
        let home = temp.path().join("home");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(home.join("Dev")).expect("create home dev");
        fs::create_dir_all(&workspace).expect("create workspace");

        let critical_chars = "x".repeat(64_000);
        fs::write(workspace.join("AGENTS.md"), &critical_chars).expect("write root");

        let mut state = empty_state();
        state.skills.push(project_skill(&workspace, "a"));

        let report = build_agents_context_report(&home, &state);
        let entry = report.entries.first().expect("entry");
        assert_eq!(entry.tokens_estimate, estimate_tokens(critical_chars.len()));
        assert_eq!(entry.severity, AgentContextSeverity::Critical);
        assert_eq!(report.totals.severity, AgentContextSeverity::Critical);
    }

    #[test]
    fn extracts_project_workspaces_from_state_deduped_and_sorted() {
        let temp = tempdir().expect("temp dir");
        let first = temp.path().join("a-workspace");
        let second = temp.path().join("b-workspace");
        fs::create_dir_all(&first).expect("create first");
        fs::create_dir_all(&second).expect("create second");

        let mut state = empty_state();
        state.skills.push(project_skill(&second, "s2"));
        state.skills.push(project_skill(&first, "s1"));
        state.skills.push(project_skill(&first, "s1-dup"));
        state.skills.push(SkillRecord {
            id: String::from("global"),
            name: String::from("Global"),
            scope: String::from("global"),
            workspace: None,
            canonical_source_path: String::new(),
            target_paths: Vec::new(),
            exists: true,
            is_symlink_canonical: false,
            package_type: String::from("dir"),
            skill_key: String::from("global"),
            symlink_target: String::new(),
            source: None,
            commit: None,
            install_status: None,
            wildcard_source: None,
            status: SkillLifecycleStatus::Active,
            archived_at: None,
            archived_bundle_path: None,
            archived_original_scope: None,
            archived_original_workspace: None,
        });

        let workspaces = project_workspaces_from_state(&state);
        assert_eq!(workspaces.len(), 2);
        assert_eq!(workspaces[0], first);
        assert_eq!(workspaces[1], second);
    }

    #[test]
    fn extracts_project_workspaces_from_non_skill_records() {
        let temp = tempdir().expect("temp dir");
        let first = temp.path().join("a-workspace");
        let second = temp.path().join("b-workspace");
        fs::create_dir_all(&first).expect("create first");
        fs::create_dir_all(&second).expect("create second");

        let mut state = empty_state();
        state.subagents.push(project_subagent(&first, "s1"));
        state.mcp_servers.push(project_mcp(&second, "m1"));

        let workspaces = project_workspaces_from_state(&state);
        assert_eq!(workspaces.len(), 2);
        assert_eq!(workspaces[0], first);
        assert_eq!(workspaces[1], second);
    }
}
