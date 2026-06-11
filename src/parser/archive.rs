use std::path::{Path, PathBuf};

use crate::domain::Entity;

use super::{display_path, is_markdown_path, parse_work_item, read_directory, ParseError};

pub fn archive_dir(workflow_dir: &Path) -> PathBuf {
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
    id_style: Option<&str>,
) -> Result<Vec<Entity>, ParseError> {
    let archive_root = archive_dir(workflow_dir);
    if !archive_root.exists() {
        return Ok(Vec::new());
    }

    let item_paths = collect_archived_item_paths(&archive_root)?;

    let mut items = Vec::with_capacity(item_paths.len());
    for item_path in item_paths {
        match parse_work_item(&item_path, allowed_statuses, id_style) {
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

fn collect_archived_item_paths(archive_root: &Path) -> Result<Vec<PathBuf>, ParseError> {
    let entries = read_directory(archive_root)?;
    let path_label = display_path(archive_root);
    let mut item_paths = Vec::new();
    for entry in entries {
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
        if is_markdown_path(&entry_path) {
            item_paths.push(entry_path);
        }
    }
    item_paths.sort();
    Ok(item_paths)
}
