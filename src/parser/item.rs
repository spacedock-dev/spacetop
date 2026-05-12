use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::domain::WorkItem;

use super::frontmatter::extract_frontmatter;
use super::{display_path, optional_text, required, ParseError};

pub fn parse_work_item(path: &Path, _allowed_statuses: &[String]) -> Result<WorkItem, ParseError> {
    let path_label = display_path(path);
    let contents = fs::read_to_string(path).map_err(|source| ParseError::ReadFile {
        path: path_label.clone(),
        source,
    })?;
    parse_work_item_contents(path, &contents, _allowed_statuses)
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
        worktree_source: None,
        main_body: None,
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
        if matches!(
            value.chars().next(),
            Some('[' | '{' | '|' | '>' | '&' | '*')
        ) {
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
