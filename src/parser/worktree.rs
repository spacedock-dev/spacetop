use std::collections::HashMap;
use std::fs;
use std::path::Path;

use sha1::{Digest, Sha1};

use crate::domain::WorkItem;

use super::{is_markdown_path, is_readme_path, parse_work_item, ParseError};

pub(crate) fn scan_worktrees(
    repo_root: &Path,
    workflow_rel: &Path,
    allowed_statuses: &[String],
) -> Result<Vec<WorkItem>, ParseError> {
    let wt_dir = repo_root.join(".worktrees");
    if !wt_dir.exists() {
        return Ok(Vec::new());
    }
    let entries = match fs::read_dir(&wt_dir) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
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
        all_items.extend(load_worktree_items(&candidate, allowed_statuses)?);
    }
    Ok(all_items)
}

fn load_worktree_items(
    workflow_dir: &Path,
    allowed_statuses: &[String],
) -> Result<Vec<WorkItem>, ParseError> {
    let item_paths = collect_worktree_item_paths(workflow_dir);
    item_paths
        .into_iter()
        .map(|item_path| parse_work_item(&item_path, allowed_statuses))
        .collect()
}

fn collect_worktree_item_paths(workflow_dir: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = fs::read_dir(workflow_dir) else {
        return Vec::new();
    };
    let mut item_paths = Vec::new();
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if is_readme_path(&entry_path) {
            continue;
        }
        if is_markdown_path(&entry_path) {
            item_paths.push(entry_path);
        }
    }
    item_paths.sort();
    item_paths
}

/// Derive the slug for a workflow entity path.
/// For folder-form entities (`{slug}/index.md`), uses the parent directory name.
/// For flat entities (`{slug}.md`), uses the file stem.
fn slug_of_path(path: &Path) -> Option<std::ffi::OsString> {
    let stem = path.file_stem()?;
    if stem == "index" {
        path.parent()
            .and_then(|p| p.file_name())
            .map(|s| s.to_owned())
    } else {
        Some(stem.to_owned())
    }
}

/// Merge main-branch items with worktree items using SHA-1 hash comparison.
/// Worktree version wins when the same slug exists in both and hashes differ.
/// Uses SHA-1 digest (not string equality on body) for content comparison (AC-5).
pub(crate) fn merge_worktree_items(
    main_items: Vec<WorkItem>,
    worktree_items: Vec<WorkItem>,
) -> Vec<WorkItem> {
    if worktree_items.is_empty() {
        return main_items;
    }
    let mut index: HashMap<std::ffi::OsString, WorkItem> = main_items
        .into_iter()
        .filter_map(|item| {
            let slug = slug_of_path(&item.path)?;
            Some((slug, item))
        })
        .collect();

    for wt_item in worktree_items {
        let Some(slug) = slug_of_path(&wt_item.path) else {
            continue;
        };
        let Some(main_item) = index.get(&slug) else {
            // Worktree-only item (AC-3).
            index.insert(slug, wt_item);
            continue;
        };
        if let Some(item) = merged_worktree_item(main_item, wt_item) {
            index.insert(slug, item);
        };
    }

    let mut result: Vec<_> = index.into_values().collect();
    result.sort_by(|a, b| {
        let a_slug = slug_of_path(&a.path);
        let b_slug = slug_of_path(&b.path);
        a_slug.cmp(&b_slug).then_with(|| a.path.cmp(&b.path))
    });
    result
}

fn merged_worktree_item(main_item: &WorkItem, wt_item: WorkItem) -> Option<WorkItem> {
    match (content_hash(&wt_item.path), content_hash(&main_item.path)) {
        (Some(wt_hash), Some(main_hash)) if wt_hash == main_hash => None,
        (None, Some(_)) => None,
        (Some(_), Some(_)) => Some(merge_main_frontmatter_with_worktree_body(
            main_item, wt_item,
        )),
        (Some(_), None) | (None, None) => Some(wt_item),
    }
}

fn content_hash(path: &Path) -> Option<[u8; 20]> {
    fs::read(path).map(|bytes| Sha1::digest(&bytes).into()).ok()
}

fn merge_main_frontmatter_with_worktree_body(main_item: &WorkItem, wt_item: WorkItem) -> WorkItem {
    WorkItem {
        path: wt_item.path,
        id: main_item.id.clone(),
        title: main_item.title.clone(),
        status: main_item.status.clone(),
        source: main_item.source.clone(),
        started: main_item.started.clone(),
        completed: main_item.completed.clone(),
        verdict: main_item.verdict.clone(),
        score: main_item.score,
        worktree: main_item.worktree.clone(),
        issue: main_item.issue.clone(),
        pr: main_item.pr.clone(),
        body: wt_item.body,
    }
}
