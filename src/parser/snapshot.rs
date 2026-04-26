use std::path::{Path, PathBuf};

use crate::domain::WorkflowSnapshot;

use super::worktree::{merge_worktree_items, scan_worktrees};
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

    let mut items = Vec::with_capacity(item_paths.len());
    for item_path in item_paths {
        items.push(parse_work_item(&item_path, &allowed_statuses)?);
    }

    let worktree_items = match path.strip_prefix(repo_root) {
        Ok(workflow_rel) => scan_worktrees(repo_root, workflow_rel, &allowed_statuses)?,
        Err(_) => Vec::new(),
    };
    let items = merge_worktree_items(items, worktree_items);

    Ok(WorkflowSnapshot { definition, items })
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
