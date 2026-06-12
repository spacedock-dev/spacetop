use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::{EntityParseError, WorkflowSnapshot};

use super::archive::archive_dir;
use super::worktree::{merge_worktree_items, scan_worktrees, slug_of_path};
use super::{
    is_markdown_path, is_readme_path, parse_work_item, parse_workflow_readme, read_directory,
    ParseError,
};

pub fn load_workflow_dir(path: &Path, repo_root: &Path) -> Result<WorkflowSnapshot, ParseError> {
    let definition = parse_workflow_readme(&path.join("README.md"))?;
    let allowed_statuses = definition
        .stages
        .iter()
        .map(|stage| stage.name.clone())
        .collect::<Vec<_>>();
    let item_paths = collect_active_item_paths(path)?;
    let archived_slugs = collect_archived_item_slugs(path);
    let id_style = definition.id_style.as_deref();

    let mut items = Vec::with_capacity(item_paths.len());
    let mut parse_errors: Vec<EntityParseError> = Vec::new();
    for item_path in item_paths {
        match parse_work_item(&item_path, &allowed_statuses, id_style) {
            Ok(item) => items.push(item),
            Err(err) if err.is_per_entity_parse_failure() => {
                parse_errors.push(entity_parse_error_from(&item_path, &err));
            }
            Err(err) => return Err(err),
        }
    }

    let (worktree_items, worktree_parse_errors) = match path.strip_prefix(repo_root) {
        Ok(workflow_rel) => scan_worktrees(repo_root, workflow_rel, &allowed_statuses, id_style)?,
        Err(_) => (Vec::new(), Vec::new()),
    };
    parse_errors.extend(worktree_parse_errors);
    let items = merge_worktree_items(items, worktree_items, &archived_slugs);

    Ok(WorkflowSnapshot {
        definition,
        items,
        parse_errors,
    })
}

pub(crate) fn entity_parse_error_from(path: &Path, err: &ParseError) -> EntityParseError {
    let (line, column) = err.yaml_location();
    EntityParseError {
        path: path.to_path_buf(),
        message: err.to_string(),
        line,
        column,
    }
}

fn collect_active_item_paths(workflow_dir: &Path) -> Result<Vec<PathBuf>, ParseError> {
    let entries = read_directory(workflow_dir)?;
    let mut item_paths = Vec::new();
    for entry in entries {
        let entry_path = entry.path();
        if is_readme_path(&entry_path) {
            continue;
        }
        if is_markdown_path(&entry_path) {
            item_paths.push(entry_path);
        }
    }
    item_paths.sort();
    Ok(item_paths)
}

fn collect_archived_item_slugs(workflow_dir: &Path) -> HashSet<OsString> {
    let archive_root = archive_dir(workflow_dir);
    let Ok(entries) = fs::read_dir(archive_root) else {
        return HashSet::new();
    };

    let mut slugs = HashSet::new();
    for entry in entries.flatten() {
        let entry_path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let entity_path = if file_type.is_dir() {
            let index_path = entry_path.join("index.md");
            index_path.is_file().then_some(index_path)
        } else if is_markdown_path(&entry_path) {
            Some(entry_path)
        } else {
            None
        };
        if let Some(slug) = entity_path.as_deref().and_then(slug_of_path) {
            slugs.insert(slug);
        }
    }
    slugs
}
