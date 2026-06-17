use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::domain::Entity;
use crate::entity_identity::entity_slug;

use super::frontmatter::{extract_frontmatter, top_level_scalar_entries};
use super::{display_path, optional_text, required, ParseError};

pub fn parse_work_item(
    path: &Path,
    allowed_statuses: &[String],
    id_style: Option<&str>,
) -> Result<Entity, ParseError> {
    let path_label = display_path(path);
    let contents = fs::read_to_string(path).map_err(|source| ParseError::ReadFile {
        path: path_label.clone(),
        source,
    })?;
    parse_work_item_contents(path, &contents, allowed_statuses, id_style)
}

fn parse_work_item_contents(
    path: &Path,
    contents: &str,
    allowed_statuses: &[String],
    id_style: Option<&str>,
) -> Result<Entity, ParseError> {
    let path_label = display_path(path);
    let (frontmatter, body) = extract_frontmatter(contents, &path_label)?;
    let raw = parse_work_item_frontmatter(frontmatter, &path_label)?;

    let id = resolve_id(raw.id, path, id_style)?;
    let title = required(raw.title, path, "title")?;
    let status = required(raw.status, path, "status")?;
    if !allowed_statuses.iter().any(|allowed| allowed == &status) {
        return Err(ParseError::UnknownStatus {
            path: path_label,
            status,
            allowed: allowed_statuses.join(", "),
        });
    }

    Ok(Entity {
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
        worktree_source: None,
        main_body: None,
    })
}

/// Resolve a work item's effective ID. A populated `id:` field always wins
/// (covers sequential workflows and slug workflows that fill it in). When the
/// field is blank and the workflow declares `id-style: slug`, identity comes
/// from the filename slug instead. Any other blank-id case keeps today's
/// `MissingRequiredField` behavior, so sequential workflows are unaffected.
fn resolve_id(
    raw_id: Option<String>,
    path: &Path,
    id_style: Option<&str>,
) -> Result<String, ParseError> {
    if let Some(id) = optional_text(raw_id) {
        return Ok(id);
    }
    if id_style == Some("slug") {
        if let Some(slug) = entity_slug(path) {
            return Ok(slug);
        }
    }
    Err(ParseError::MissingRequiredField {
        path: display_path(path),
        field: "id",
    })
}

fn parse_work_item_frontmatter(
    frontmatter: &str,
    path_label: &str,
) -> Result<RawWorkItemFrontmatter, ParseError> {
    match serde_yaml::from_str(frontmatter) {
        Ok(raw) => Ok(raw),
        Err(source) => {
            parse_flat_work_item_frontmatter(frontmatter).ok_or(ParseError::MalformedYaml {
                path: path_label.to_string(),
                source,
            })
        }
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

    for (key, value) in top_level_scalar_entries(frontmatter)? {
        match key {
            "id" => raw.id = optional_text(Some(value)),
            "title" => raw.title = optional_text(Some(value)),
            "status" => raw.status = optional_text(Some(value)),
            "source" => raw.source = optional_text(Some(value)),
            "started" => raw.started = optional_text(Some(value)),
            "completed" => raw.completed = optional_text(Some(value)),
            "verdict" => raw.verdict = optional_text(Some(value)),
            "score" => {
                raw.score = if value.trim().is_empty() {
                    None
                } else {
                    Some(value.parse().ok()?)
                }
            }
            "worktree" => raw.worktree = optional_text(Some(value)),
            "issue" => raw.issue = optional_text(Some(value)),
            "pr" => raw.pr = optional_text(Some(value)),
            _ => {}
        }
    }

    Some(raw)
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
