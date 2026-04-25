use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use sha1::{Digest, Sha1};
use thiserror::Error;

use crate::domain::{StageDefinition, WorkItem, WorkflowDefinition, WorkflowSnapshot};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("{path}: failed to read file: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: failed to read directory: {source}")]
    ReadDirectory {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: missing YAML frontmatter delimited by ---")]
    MissingFrontmatter { path: String },
    #[error("{path}: unterminated YAML frontmatter delimited by ---")]
    UnterminatedFrontmatter { path: String },
    #[error("{path}: malformed YAML frontmatter: {source}")]
    MalformedYaml {
        path: String,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("{path}: missing required field '{field}'")]
    MissingRequiredField { path: String, field: &'static str },
    #[error("{path}: unknown status '{status}'; allowed statuses: {allowed}")]
    UnknownStatus {
        path: String,
        status: String,
        allowed: String,
    },
}

pub fn parse_workflow_readme(path: &Path) -> Result<WorkflowDefinition, ParseError> {
    let path_label = display_path(path);
    let contents = fs::read_to_string(path).map_err(|source| ParseError::ReadFile {
        path: path_label.clone(),
        source,
    })?;
    let (frontmatter, _) = extract_frontmatter(&contents, &path_label)?;
    let raw: RawWorkflowFrontmatter =
        serde_yaml::from_str(frontmatter).map_err(|source| ParseError::MalformedYaml {
            path: path_label.clone(),
            source,
        })?;

    let stage_block = raw.stages.ok_or(ParseError::MissingRequiredField {
        path: path_label,
        field: "stages",
    })?;
    let defaults = stage_block.defaults.unwrap_or_default();
    let mut stages = Vec::with_capacity(stage_block.states.len());
    for raw_stage in stage_block.states {
        let name = required(raw_stage.name, path, "stages.states.name")?;
        stages.push(StageDefinition {
            name,
            initial: raw_stage
                .initial
                .unwrap_or(defaults.initial.unwrap_or(false)),
            terminal: raw_stage
                .terminal
                .unwrap_or(defaults.terminal.unwrap_or(false)),
            gate: raw_stage.gate.unwrap_or(defaults.gate.unwrap_or(false)),
            fresh: raw_stage.fresh.unwrap_or(defaults.fresh.unwrap_or(false)),
            feedback_to: raw_stage.feedback_to.or(defaults.feedback_to.clone()),
            worktree: raw_stage
                .worktree
                .unwrap_or(defaults.worktree.unwrap_or(false)),
            concurrency: raw_stage.concurrency.or(defaults.concurrency),
        });
    }

    let stage_colors = crate::domain::assign_stage_colors(&stages);
    Ok(WorkflowDefinition {
        root: path.parent().unwrap_or_else(|| Path::new("")).to_path_buf(),
        stages,
        id_style: raw.id_style,
        entity_type: raw.entity_type,
        entity_label: raw.entity_label,
        entity_label_plural: raw.entity_label_plural,
        stage_colors,
    })
}

pub fn parse_work_item(path: &Path, _allowed_statuses: &[String]) -> Result<WorkItem, ParseError> {
    let path_label = display_path(path);
    let contents = fs::read_to_string(path).map_err(|source| ParseError::ReadFile {
        path: path_label.clone(),
        source,
    })?;
    parse_work_item_contents(path, &contents, _allowed_statuses)
}

/// Return the `_archive/` directory for the workflow.
pub fn archive_dir(workflow_dir: &Path) -> std::path::PathBuf {
    workflow_dir.join("_archive")
}

/// Load archived work items from `_archive/*.md` and `_archive/*/index.md`.
///
/// - Missing `_archive/` directory returns `Ok(Vec::new())`.
/// - Folder entities are picked up via `_archive/<dir>/index.md`; nested
///   sibling markdown files inside a folder entity are ignored.
/// - Results are sorted newest-first by `completed` timestamp. Items with
///   no `completed` timestamp sort last; within that group, filename
///   ordering (ascending) is used as a deterministic tiebreaker.
pub fn load_archived_items(
    workflow_dir: &Path,
    allowed_statuses: &[String],
) -> Result<Vec<WorkItem>, ParseError> {
    let archive_root = archive_dir(workflow_dir);
    if !archive_root.exists() {
        return Ok(Vec::new());
    }

    let path_label = display_path(&archive_root);
    let mut item_paths = Vec::new();
    for entry in fs::read_dir(&archive_root).map_err(|source| ParseError::ReadDirectory {
        path: path_label.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| ParseError::ReadDirectory {
            path: path_label.clone(),
            source,
        })?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| ParseError::ReadDirectory {
                path: path_label.clone(),
                source,
            })?;
        if file_type.is_dir() {
            let index_path = entry_path.join("index.md");
            if index_path.is_file() {
                item_paths.push(index_path);
            }
            continue;
        }
        if entry_path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("md")
        {
            item_paths.push(entry_path);
        }
    }
    item_paths.sort();

    let mut items = Vec::with_capacity(item_paths.len());
    for item_path in item_paths {
        match parse_work_item(&item_path, allowed_statuses) {
            Ok(item) => items.push(item),
            Err(err) if should_skip_archived_parse_error(&err) => continue,
            Err(err) => return Err(err),
        }
    }

    items.sort_by(
        |a, b| match (a.completed.as_deref(), b.completed.as_deref()) {
            (Some(ac), Some(bc)) => bc.cmp(ac).then_with(|| a.path.cmp(&b.path)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.path.cmp(&b.path),
        },
    );

    Ok(items)
}

fn should_skip_archived_parse_error(error: &ParseError) -> bool {
    matches!(
        error,
        ParseError::MissingFrontmatter { .. }
            | ParseError::UnterminatedFrontmatter { .. }
            | ParseError::MalformedYaml { .. }
            | ParseError::MissingRequiredField { .. }
            | ParseError::UnknownStatus { .. }
    )
}

pub fn load_workflow_dir(path: &Path, repo_root: &Path) -> Result<WorkflowSnapshot, ParseError> {
    let definition = parse_workflow_readme(&path.join("README.md"))?;
    let allowed_statuses = definition
        .stages
        .iter()
        .map(|stage| stage.name.clone())
        .collect::<Vec<_>>();
    let path_label = display_path(path);
    let mut item_paths = Vec::new();
    for entry in fs::read_dir(path).map_err(|source| ParseError::ReadDirectory {
        path: path_label.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| ParseError::ReadDirectory {
            path: path_label.clone(),
            source,
        })?;
        let entry_path = entry.path();
        if entry_path.file_name().and_then(|name| name.to_str()) == Some("README.md") {
            continue;
        }
        if entry_path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("md")
        {
            item_paths.push(entry_path);
        }
    }
    item_paths.sort();

    let mut items = Vec::with_capacity(item_paths.len());
    for item_path in item_paths {
        items.push(parse_work_item(&item_path, &allowed_statuses)?);
    }

    let workflow_rel = path.strip_prefix(repo_root).unwrap_or(path);
    let worktree_items = scan_worktrees(repo_root, workflow_rel, &allowed_statuses);
    let items = merge_worktree_items(items, worktree_items);

    Ok(WorkflowSnapshot { definition, items })
}

/// Scan `.worktrees/*/` subdirectories under `repo_root` for workflow entity
/// files at `workflow_rel`. Returns all successfully parsed items from all
/// worktrees; silently skips worktrees that do not contain the workflow dir.
fn scan_worktrees(
    repo_root: &Path,
    workflow_rel: &Path,
    allowed_statuses: &[String],
) -> Vec<crate::domain::WorkItem> {
    let wt_dir = repo_root.join(".worktrees");
    if !wt_dir.exists() {
        return Vec::new();
    }
    let entries = match fs::read_dir(&wt_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut all_items = Vec::new();
    for entry in entries.flatten() {
        let wt_root = entry.path();
        if !wt_root.is_dir() {
            continue;
        }
        let candidate = wt_root.join(workflow_rel);
        if !candidate.is_dir() {
            continue;
        }
        let mut item_paths = Vec::new();
        let Ok(dir_entries) = fs::read_dir(&candidate) else {
            continue;
        };
        for file_entry in dir_entries.flatten() {
            let file_path = file_entry.path();
            if file_path.file_name().and_then(|n| n.to_str()) == Some("README.md") {
                continue;
            }
            if file_path
                .extension()
                .and_then(|e| e.to_str())
                == Some("md")
            {
                item_paths.push(file_path);
            }
        }
        item_paths.sort();
        for item_path in item_paths {
            if let Ok(item) = parse_work_item(&item_path, allowed_statuses) {
                all_items.push(item);
            }
        }
    }
    all_items
}

/// Merge main-branch items with worktree items using SHA-1 hash comparison.
/// Worktree version wins when the same slug exists in both and hashes differ.
/// Uses SHA-1 digest (not string equality on body) for content comparison (AC-5).
fn merge_worktree_items(
    main_items: Vec<crate::domain::WorkItem>,
    worktree_items: Vec<crate::domain::WorkItem>,
) -> Vec<crate::domain::WorkItem> {
    if worktree_items.is_empty() {
        return main_items;
    }
    let mut index: HashMap<String, crate::domain::WorkItem> = main_items
        .into_iter()
        .filter_map(|item| {
            let slug = item
                .path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())?;
            Some((slug, item))
        })
        .collect();

    for wt_item in worktree_items {
        let Some(slug) = wt_item
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
        else {
            continue;
        };
        if let Some(main_item) = index.get(&slug) {
            // Both exist: compare via SHA-1 digest — not string equality on body (AC-5).
            let wt_hash = fs::read(&wt_item.path)
                .map(|b| Sha1::digest(&b))
                .ok();
            let main_hash = fs::read(&main_item.path)
                .map(|b| Sha1::digest(&b))
                .ok();
            match (wt_hash, main_hash) {
                (Some(wh), Some(mh)) if wh == mh => {
                    // Hashes match; keep main copy (already in index).
                }
                _ => {
                    // Hashes differ or IO error: worktree wins (AC-4).
                    index.insert(slug, wt_item);
                }
            }
        } else {
            // Worktree-only item (AC-3).
            index.insert(slug, wt_item);
        }
    }

    let mut result: Vec<_> = index.into_values().collect();
    result.sort_by(|a, b| a.path.cmp(&b.path));
    result
}

fn parse_work_item_contents(
    path: &Path,
    contents: &str,
    allowed_statuses: &[String],
) -> Result<WorkItem, ParseError> {
    let path_label = display_path(path);
    let (frontmatter, body) = extract_frontmatter(contents, &path_label)?;
    let raw = parse_work_item_frontmatter(frontmatter, &path_label)?;

    let id = required(raw.id, path, "id")?;
    let title = required(raw.title, path, "title")?;
    let status = required(raw.status, path, "status")?;
    if !allowed_statuses.iter().any(|allowed| allowed == &status) {
        return Err(ParseError::UnknownStatus {
            path: path_label,
            status,
            allowed: allowed_statuses.join(", "),
        });
    }

    Ok(WorkItem {
        path: path.to_path_buf(),
        id,
        title,
        status,
        source: optional_text(raw.source),
        started: optional_text(raw.started),
        completed: optional_text(raw.completed),
        verdict: optional_text(raw.verdict),
        score: raw.score,
        worktree: optional_text(raw.worktree),
        issue: optional_text(raw.issue),
        pr: optional_text(raw.pr),
        body: body.to_string(),
    })
}

fn parse_work_item_frontmatter(
    frontmatter: &str,
    path_label: &str,
) -> Result<RawWorkItemFrontmatter, ParseError> {
    match serde_yaml::from_str(frontmatter) {
        Ok(raw) => Ok(raw),
        Err(source) => parse_flat_work_item_frontmatter(frontmatter).ok_or(ParseError::MalformedYaml {
            path: path_label.to_string(),
            source,
        }),
    }
}

fn parse_flat_work_item_frontmatter(frontmatter: &str) -> Option<RawWorkItemFrontmatter> {
    let mut raw = RawWorkItemFrontmatter {
        id: None,
        title: None,
        status: None,
        source: None,
        started: None,
        completed: None,
        verdict: None,
        score: None,
        worktree: None,
        issue: None,
        pr: None,
    };

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            return None;
        }
        let (key, value) = trimmed.split_once(':')?;
        let key = key.trim();
        if key.is_empty() {
            return None;
        }
        let value = value.trim_start();
        if matches!(value.chars().next(), Some('[' | '{' | '|' | '>' | '&' | '*')) {
            return None;
        }
        let value = unquote_scalar(value);
        match key {
            "id" => raw.id = optional_text(Some(value.to_string())),
            "title" => raw.title = optional_text(Some(value.to_string())),
            "status" => raw.status = optional_text(Some(value.to_string())),
            "source" => raw.source = optional_text(Some(value.to_string())),
            "started" => raw.started = optional_text(Some(value.to_string())),
            "completed" => raw.completed = optional_text(Some(value.to_string())),
            "verdict" => raw.verdict = optional_text(Some(value.to_string())),
            "score" => {
                raw.score = if value.trim().is_empty() {
                    None
                } else {
                    Some(value.parse().ok()?)
                }
            }
            "worktree" => raw.worktree = optional_text(Some(value.to_string())),
            "issue" => raw.issue = optional_text(Some(value.to_string())),
            "pr" => raw.pr = optional_text(Some(value.to_string())),
            _ => {}
        }
    }

    Some(raw)
}

fn unquote_scalar(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn extract_frontmatter<'a>(
    contents: &'a str,
    path: &str,
) -> Result<(&'a str, &'a str), ParseError> {
    match split_frontmatter(contents) {
        Some(SplitFrontmatter::Ok { frontmatter, body }) => Ok((frontmatter, body)),
        Some(SplitFrontmatter::Unterminated) => Err(ParseError::UnterminatedFrontmatter {
            path: path.to_string(),
        }),
        None => Err(ParseError::MissingFrontmatter {
            path: path.to_string(),
        }),
    }
}

pub(crate) enum SplitFrontmatter<'a> {
    Ok { frontmatter: &'a str, body: &'a str },
    Unterminated,
}

/// Split a markdown file's text into its YAML frontmatter block (sans `---` fences)
/// and the body. Returns `None` when no opening `---` fence is present on the first line.
pub(crate) fn split_frontmatter(contents: &str) -> Option<SplitFrontmatter<'_>> {
    let rest = contents
        .strip_prefix("---\r\n")
        .or_else(|| contents.strip_prefix("---\n"))?;
    let body_start = contents.len() - rest.len();

    let remaining = &contents[body_start..];
    let Some(relative_end) = remaining.find("\n---") else {
        return Some(SplitFrontmatter::Unterminated);
    };
    let closing_start = body_start + relative_end + 1;
    let after_marker = closing_start + 3;
    let after_marker = if contents[after_marker..].starts_with("\r\n") {
        after_marker + 2
    } else if contents[after_marker..].starts_with('\n') {
        after_marker + 1
    } else {
        after_marker
    };
    let body = contents[after_marker..]
        .strip_prefix("\r\n")
        .or_else(|| contents[after_marker..].strip_prefix('\n'))
        .unwrap_or(&contents[after_marker..]);

    Some(SplitFrontmatter::Ok {
        frontmatter: &contents[body_start..closing_start],
        body,
    })
}

fn required(value: Option<String>, path: &Path, field: &'static str) -> Result<String, ParseError> {
    let Some(value) = optional_text(value) else {
        return Err(ParseError::MissingRequiredField {
            path: display_path(path),
            field,
        });
    };
    Ok(value)
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[derive(Debug, Deserialize)]
struct RawWorkflowFrontmatter {
    #[serde(rename = "id-style")]
    id_style: Option<String>,
    #[serde(rename = "entity-type")]
    entity_type: Option<String>,
    #[serde(rename = "entity-label")]
    entity_label: Option<String>,
    #[serde(rename = "entity-label-plural")]
    entity_label_plural: Option<String>,
    stages: Option<RawStageBlock>,
}

#[derive(Debug, Deserialize)]
struct RawStageBlock {
    defaults: Option<RawStageDefaults>,
    states: Vec<RawStage>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawStageDefaults {
    initial: Option<bool>,
    terminal: Option<bool>,
    gate: Option<bool>,
    fresh: Option<bool>,
    #[serde(rename = "feedback-to")]
    feedback_to: Option<String>,
    worktree: Option<bool>,
    concurrency: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawStage {
    name: Option<String>,
    initial: Option<bool>,
    terminal: Option<bool>,
    gate: Option<bool>,
    fresh: Option<bool>,
    #[serde(rename = "feedback-to")]
    feedback_to: Option<String>,
    worktree: Option<bool>,
    concurrency: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawWorkItemFrontmatter {
    id: Option<String>,
    title: Option<String>,
    status: Option<String>,
    source: Option<String>,
    started: Option<String>,
    completed: Option<String>,
    verdict: Option<String>,
    score: Option<f64>,
    worktree: Option<String>,
    issue: Option<String>,
    pr: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        load_archived_items, load_workflow_dir, parse_work_item, parse_workflow_readme,
        ParseError,
    };

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev")
    }

    fn write_temp_markdown(name: &str, contents: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("spacetop-parser-test-{unique}"));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let path = dir.join(name);
        fs::write(&path, contents).expect("temp markdown should be written");
        path
    }

    fn stage_names(root: &Path) -> Vec<String> {
        parse_workflow_readme(&root.join("README.md"))
            .expect("workflow README should parse")
            .stages
            .into_iter()
            .map(|stage| stage.name)
            .collect()
    }

    #[test]
    fn parses_workflow_readme_stage_metadata_with_defaults_and_overrides() {
        let root = fixture_root();
        let workflow =
            parse_workflow_readme(&root.join("README.md")).expect("workflow README should parse");

        assert_eq!(workflow.root, root);
        assert_eq!(workflow.id_style.as_deref(), Some("sequential"));
        assert_eq!(workflow.entity_type.as_deref(), Some("development_task"));
        assert_eq!(
            workflow
                .stages
                .iter()
                .map(|stage| stage.name.as_str())
                .collect::<Vec<_>>(),
            ["design", "plan", "implement", "review", "done"]
        );

        let design = workflow
            .stages
            .iter()
            .find(|stage| stage.name == "design")
            .expect("design stage should exist");
        assert!(design.initial);
        assert!(!design.terminal);
        assert_eq!(design.concurrency, Some(2));

        let implement = workflow
            .stages
            .iter()
            .find(|stage| stage.name == "implement")
            .expect("implement stage should exist");
        assert!(implement.worktree);
        assert_eq!(implement.concurrency, Some(2));

        let review = workflow
            .stages
            .iter()
            .find(|stage| stage.name == "review")
            .expect("review stage should exist");
        assert!(review.gate);
        assert!(review.fresh);
        assert_eq!(review.feedback_to.as_deref(), Some("implement"));

        let done = workflow
            .stages
            .iter()
            .find(|stage| stage.name == "done")
            .expect("done stage should exist");
        assert!(done.terminal);
    }

    #[test]
    fn parses_work_item_frontmatter_and_preserves_markdown_body() {
        let root = fixture_root();
        let allowed_statuses = stage_names(&root);
        let path = write_temp_markdown(
            "work-item.md",
            r#"---
id: "002"
title: Parse Spacedock Workflow Files
status: implement
source: commission seed
score: 1.0
worktree: .worktrees/spacedock-ensign-parse-spacedock-workflow-files
---

Read Spacedock workflow files into typed models.

## Acceptance criteria

Body text should be preserved without frontmatter.
"#,
        );
        let item = parse_work_item(&path, &allowed_statuses).expect("work item should parse");

        assert_eq!(item.id, "002");
        assert_eq!(item.title, "Parse Spacedock Workflow Files");
        assert_eq!(item.status, "implement");
        assert_eq!(item.source.as_deref(), Some("commission seed"));
        assert_eq!(item.score, Some(1.0));
        assert_eq!(
            item.worktree.as_deref(),
            Some(".worktrees/spacedock-ensign-parse-spacedock-workflow-files")
        );
        assert!(item.body.starts_with("Read Spacedock workflow"));
        assert!(item.body.contains("## Acceptance criteria"));
        assert!(!item.body.starts_with("---"));
    }

    #[test]
    fn loads_workflow_snapshot_from_directory_ignoring_mods_and_archive() {
        let root = unique_temp_dir("snapshot");
        fs::copy(fixture_root().join("README.md"), root.join("README.md"))
            .expect("README fixture should copy");
        write_markdown(
            &root.join("active.md"),
            r#"---
id: "001"
title: Active
status: design
---

Active body.
"#,
        );
        write_markdown(
            &root.join("_mods/ignored.md"),
            r#"---
id: "002"
title: Ignored Mod
status: design
---

Ignored.
"#,
        );
        write_markdown(
            &root.join("_archive/archived.md"),
            r#"---
id: "003"
title: Ignored Archived
status: done
---

Ignored.
"#,
        );
        let snapshot = load_workflow_dir(&root, &root).expect("workflow directory should load");

        assert_eq!(snapshot.definition.stages.len(), 5);
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].title, "Active");
        assert!(snapshot
            .items
            .iter()
            .all(|item| !item.path.components().any(|component| {
                let value = component.as_os_str();
                value == "_mods" || value == "_archive"
            })));
        let allowed_statuses = snapshot
            .definition
            .stages
            .iter()
            .map(|stage| stage.name.as_str())
            .collect::<Vec<_>>();
        assert!(snapshot
            .items
            .iter()
            .all(|item| allowed_statuses.contains(&item.status.as_str())));
    }

    #[test]
    fn missing_frontmatter_error_names_file_and_context() {
        let path = write_temp_markdown("missing.md", "# Missing\n");
        let error = parse_work_item(&path, &["design".to_string()])
            .expect_err("missing frontmatter should fail")
            .to_string();

        assert!(error.contains("missing YAML frontmatter"));
        assert!(error.contains("missing.md"));
    }

    #[test]
    fn unknown_status_error_includes_value_and_allowed_context() {
        let path = write_temp_markdown(
            "unknown.md",
            r#"---
id: "999"
title: Unknown Status
status: impossible
---

Body
"#,
        );
        let error = parse_work_item(&path, &["design".to_string(), "done".to_string()])
            .expect_err("unknown status should fail")
            .to_string();

        assert!(error.contains("unknown status 'impossible'"));
        assert!(error.contains("allowed statuses: design, done"));
    }

    #[test]
    fn malformed_yaml_error_is_distinct_from_validation_errors() {
        let path = write_temp_markdown(
            "malformed.md",
            r#"---
id: [
---

Body
"#,
        );
        let error = parse_work_item(&path, &["design".to_string()])
            .expect_err("malformed YAML should fail")
            .to_string();

        assert!(error.contains("malformed YAML frontmatter"));
        assert!(error.contains("malformed.md"));
    }

    #[test]
    fn parses_flat_frontmatter_with_unquoted_colon_in_title() {
        let path = write_temp_markdown(
            "colon-title.md",
            r#"---
id: 132
title: Codex first officer: derive reusable context visibility
status: ideation
source: FO observation
score: 0.62
---

Body
"#,
        );
        let item = parse_work_item(
            &path,
            &["backlog".to_string(), "ideation".to_string(), "done".to_string()],
        )
        .expect("flat frontmatter fallback should parse");

        assert_eq!(item.id, "132");
        assert_eq!(
            item.title,
            "Codex first officer: derive reusable context visibility"
        );
        assert_eq!(item.status, "ideation");
        assert_eq!(item.source.as_deref(), Some("FO observation"));
        assert_eq!(item.score, Some(0.62));
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("spacetop-archive-{label}-{unique}"));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    fn write_markdown(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir should be created");
        }
        fs::write(path, contents).expect("markdown should be written");
    }

    #[test]
    fn load_archived_items_returns_entries_from_flat_files() {
        let root = fixture_root();
        let allowed = stage_names(&root);
        let items = load_archived_items(&root, &allowed).expect("archive should load");

        assert!(items.len() >= 3, "expected at least 3 archived entries");
        let titles: Vec<&str> = items.iter().map(|item| item.title.as_str()).collect();
        assert!(titles.contains(&"Scaffold Rust CLI Project"));
        assert!(titles.contains(&"Parse Spacedock Workflow Files"));
        assert!(titles.contains(&"Build Initial TUI Overview"));
        assert!(items.iter().all(|item| item.status == "done"));
    }

    #[test]
    fn load_archived_items_sorts_by_completed_desc_with_missing_last() {
        let dir = unique_temp_dir("sort");
        let archive = dir.join("_archive");
        fs::create_dir_all(&archive).expect("archive dir");

        write_markdown(
            &archive.join("early.md"),
            r#"---
id: "001"
title: Early
status: done
completed: 2026-04-24T14:49:53Z
---

Body
"#,
        );
        write_markdown(
            &archive.join("late.md"),
            r#"---
id: "002"
title: Late
status: done
completed: 2026-04-24T15:00:00Z
---

Body
"#,
        );
        write_markdown(
            &archive.join("unknown.md"),
            r#"---
id: "003"
title: Unknown
status: done
---

Body
"#,
        );

        let items = load_archived_items(&dir, &["done".to_string()]).expect("archive load");
        let titles: Vec<&str> = items.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, vec!["Late", "Early", "Unknown"]);
    }

    #[test]
    fn load_archived_items_reads_folder_entity_index_md() {
        let dir = unique_temp_dir("folder");
        let archive = dir.join("_archive");
        let entity = archive.join("foo");
        fs::create_dir_all(&entity).expect("entity dir");

        write_markdown(
            &entity.join("index.md"),
            r#"---
id: "010"
title: Folder Entity
status: done
completed: 2026-04-24T10:00:00Z
---

Body
"#,
        );
        write_markdown(
            &entity.join("notes.md"),
            r#"---
id: "011"
title: Should Be Ignored
status: done
---

Body
"#,
        );

        let items = load_archived_items(&dir, &["done".to_string()]).expect("archive load");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Folder Entity");
    }

    #[test]
    fn load_archived_items_missing_archive_dir_is_empty_ok() {
        let dir = unique_temp_dir("missing");
        let items = load_archived_items(&dir, &["done".to_string()]).expect("should be Ok");
        assert!(items.is_empty());
    }

    #[test]
    fn load_archived_items_returns_empty_when_all_entries_are_malformed() {
        let dir = unique_temp_dir("broken");
        let archive = dir.join("_archive");
        fs::create_dir_all(&archive).expect("archive dir");
        write_markdown(
            &archive.join("broken.md"),
            r#"---
id: [
---

Body
"#,
        );

        let items = load_archived_items(&dir, &["done".to_string()]).expect("archive load");
        assert!(items.is_empty());
    }

    #[test]
    fn load_archived_items_skips_malformed_entries_and_keeps_valid_ones() {
        let dir = unique_temp_dir("archive-skip-broken");
        let archive = dir.join("_archive");
        fs::create_dir_all(&archive).expect("archive dir");
        write_markdown(
            &archive.join("good.md"),
            r#"---
id: "001"
title: Good
status: done
completed: 2026-04-24T15:00:00Z
---

Body
"#,
        );
        write_markdown(
            &archive.join("broken.md"),
            r#"---
id: 131
title: Broken: archive entry
<<<<<<< HEAD
status: validation
=======
status: done
>>>>>>> branch
---

Body
"#,
        );

        let items = load_archived_items(&dir, &["done".to_string()]).expect("archive load");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Good");
    }

    #[cfg(unix)]
    #[test]
    fn load_archived_items_returns_io_errors_instead_of_silently_skipping_them() {
        let dir = unique_temp_dir("archive-io-error");
        let archive = dir.join("_archive");
        fs::create_dir_all(&archive).expect("archive dir");
        std::os::unix::fs::symlink(archive.join("missing-target.md"), archive.join("broken.md"))
            .expect("symlink");

        let err =
            load_archived_items(&dir, &["done".to_string()]).expect_err("archive load should fail");
        assert!(
            matches!(err, ParseError::ReadFile { .. }),
            "expected ReadFile error, got {err:?}"
        );
    }

    #[test]
    fn missing_required_work_item_field_error_names_field() {
        let path = write_temp_markdown(
            "missing-title.md",
            r#"---
id: "999"
status: design
---

Body
"#,
        );
        let error = parse_work_item(&path, &["design".to_string()])
            .expect_err("missing title should fail")
            .to_string();

        assert!(error.contains("missing required field 'title'"));
        assert!(error.contains("missing-title.md"));
    }

    // ---- Worktree scan tests (AC-1 through AC-4, AC-6, AC-7) ----

    /// Write a minimal workflow README and an optional entity file into `dir`.
    fn write_minimal_workflow(dir: &Path, entity_name: Option<&str>, entity_content: Option<&str>) {
        fs::create_dir_all(dir).expect("workflow dir");
        fs::write(
            dir.join("README.md"),
            "---\ncommissioned-by: spacedock@0.10.1\nstages:\n  states:\n    - name: design\n      initial: true\n    - name: done\n      terminal: true\n---\n\n# Workflow\n",
        )
        .expect("write README");
        if let (Some(name), Some(content)) = (entity_name, entity_content) {
            write_markdown(&dir.join(name), content);
        }
    }

    fn entity_md(id: &str, title: &str) -> String {
        format!(
            "---\nid: \"{id}\"\ntitle: {title}\nstatus: design\n---\n\n{title} body.\n"
        )
    }

    #[test]
    fn worktree_items_included() {
        // AC-1, AC-6: two worktrees each with a distinct entity
        let root = unique_temp_dir("wt-included");
        let wf = root.join("docs/wf");
        write_minimal_workflow(&wf, Some("main-task.md"), Some(&entity_md("001", "Main Task")));
        let wt_a = root.join(".worktrees/wt-a/docs/wf");
        write_minimal_workflow(&wt_a, Some("task-a.md"), Some(&entity_md("002", "Task A")));
        let wt_b = root.join(".worktrees/wt-b/docs/wf");
        write_minimal_workflow(&wt_b, Some("task-b.md"), Some(&entity_md("003", "Task B")));

        let snapshot = load_workflow_dir(&wf, &root).expect("load workflow dir");
        let titles: Vec<&str> = snapshot.items.iter().map(|i| i.title.as_str()).collect();
        assert!(titles.contains(&"Main Task"), "main task missing: {titles:?}");
        assert!(titles.contains(&"Task A"), "task-a missing: {titles:?}");
        assert!(titles.contains(&"Task B"), "task-b missing: {titles:?}");
        assert_eq!(snapshot.items.len(), 3);
    }

    #[test]
    fn main_only_items_preserved() {
        // AC-2: main has aaa, worktree has bbb — both appear
        let root = unique_temp_dir("main-only");
        let wf = root.join("docs/wf");
        write_minimal_workflow(&wf, Some("aaa.md"), Some(&entity_md("001", "AAA")));
        let wt = root.join(".worktrees/wt-1/docs/wf");
        write_minimal_workflow(&wt, Some("bbb.md"), Some(&entity_md("002", "BBB")));

        let snapshot = load_workflow_dir(&wf, &root).expect("load");
        let titles: Vec<&str> = snapshot.items.iter().map(|i| i.title.as_str()).collect();
        assert!(titles.contains(&"AAA"), "main-only item dropped: {titles:?}");
        assert!(titles.contains(&"BBB"), "worktree-only item missing: {titles:?}");
    }

    #[test]
    fn worktree_only_items_shown() {
        // AC-3: main has no entity files; worktree has ccc.md
        let root = unique_temp_dir("wt-only");
        let wf = root.join("docs/wf");
        write_minimal_workflow(&wf, None, None);
        let wt = root.join(".worktrees/wt-1/docs/wf");
        write_minimal_workflow(&wt, Some("ccc.md"), Some(&entity_md("003", "CCC")));

        let snapshot = load_workflow_dir(&wf, &root).expect("load");
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].title, "CCC");
    }

    #[test]
    fn worktree_version_wins_on_hash_mismatch() {
        // AC-4: same slug in main and worktree with different content
        let root = unique_temp_dir("wt-wins");
        let wf = root.join("docs/wf");
        write_minimal_workflow(
            &wf,
            Some("task.md"),
            Some(&entity_md("010", "Main Version")),
        );
        let wt = root.join(".worktrees/wt-1/docs/wf");
        write_minimal_workflow(
            &wt,
            Some("task.md"),
            Some(&entity_md("010", "Worktree Version")),
        );

        let snapshot = load_workflow_dir(&wf, &root).expect("load");
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].title, "Worktree Version");
    }

    #[test]
    fn no_regression_without_worktrees() {
        // AC-7: no .worktrees directory — behavior identical to before
        let root = unique_temp_dir("no-wt");
        let wf = root.join("docs/wf");
        write_minimal_workflow(&wf, Some("solo.md"), Some(&entity_md("001", "Solo")));
        // Confirm no .worktrees dir exists
        assert!(!root.join(".worktrees").exists());

        let snapshot = load_workflow_dir(&wf, &root).expect("load");
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].title, "Solo");
    }

    #[test]
    fn same_content_hash_keeps_main_item_path() {
        // AC-4 inverse: same content hash → main item is kept (either is fine per spec)
        let root = unique_temp_dir("same-hash");
        let content = entity_md("020", "Identical");
        let wf = root.join("docs/wf");
        write_minimal_workflow(&wf, Some("task.md"), Some(&content));
        let wt = root.join(".worktrees/wt-1/docs/wf");
        write_minimal_workflow(&wt, Some("task.md"), Some(&content));

        let snapshot = load_workflow_dir(&wf, &root).expect("load");
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].title, "Identical");
        // When hashes match, the main copy is retained.
        assert!(snapshot.items[0].path.starts_with(&wf));
    }
}
