use std::fs;
use std::path::Path;

use serde::Deserialize;
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

    Ok(WorkflowDefinition {
        root: path.parent().unwrap_or_else(|| Path::new("")).to_path_buf(),
        stages,
        id_style: raw.id_style,
        entity_type: raw.entity_type,
        entity_label: raw.entity_label,
        entity_label_plural: raw.entity_label_plural,
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

pub fn load_workflow_dir(path: &Path) -> Result<WorkflowSnapshot, ParseError> {
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

    Ok(WorkflowSnapshot { definition, items })
}

fn parse_work_item_contents(
    path: &Path,
    contents: &str,
    allowed_statuses: &[String],
) -> Result<WorkItem, ParseError> {
    let path_label = display_path(path);
    let (frontmatter, body) = extract_frontmatter(contents, &path_label)?;
    let raw: RawWorkItemFrontmatter =
        serde_yaml::from_str(frontmatter).map_err(|source| ParseError::MalformedYaml {
            path: path_label.clone(),
            source,
        })?;

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

fn extract_frontmatter<'a>(
    contents: &'a str,
    path: &str,
) -> Result<(&'a str, &'a str), ParseError> {
    let body_start = if let Some(rest) = contents.strip_prefix("---\r\n") {
        contents.len() - rest.len()
    } else if let Some(rest) = contents.strip_prefix("---\n") {
        contents.len() - rest.len()
    } else {
        return Err(ParseError::MissingFrontmatter {
            path: path.to_string(),
        });
    };

    let remaining = &contents[body_start..];
    let Some(relative_end) = remaining.find("\n---") else {
        return Err(ParseError::UnterminatedFrontmatter {
            path: path.to_string(),
        });
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

    Ok((&contents[body_start..closing_start], body))
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
    value.and_then(|text| {
        let trimmed = text.trim();
        (!trimmed.is_empty()).then(|| text)
    })
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

    use super::{load_workflow_dir, parse_work_item, parse_workflow_readme};

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
        let root = fixture_root();
        let snapshot = load_workflow_dir(&root).expect("workflow directory should load");

        assert_eq!(snapshot.definition.stages.len(), 5);
        assert!(!snapshot.items.is_empty());
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
}
