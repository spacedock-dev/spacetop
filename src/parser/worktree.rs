use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};

use crate::domain::{Entity, EntityParseError};

use super::snapshot::entity_parse_error_from;
use super::{is_markdown_path, is_readme_path, parse_work_item, ParseError};

/// Return the union of worktree root directories that may host a mirrored
/// workflow tree: `<repo_root>/.worktrees/*` and
/// `<repo_root>/.claude/worktrees/*`. Both conventions are scanned because
/// either may be in use on a given checkout. Missing parent directories are
/// not errors — the helper returns whatever entries exist.
///
/// Ordering is deterministic: `.worktrees/*` is scanned before
/// `.claude/worktrees/*`, and within each parent the children are sorted
/// lexicographically by path. Because [`merge_worktree_items`] overwrites
/// existing entries by slug as it iterates, later roots win — so given a
/// slug present in both conventions, the `.claude/worktrees/*` copy wins,
/// and within a parent the lexicographically-greatest child wins. This
/// guarantees stable merges across platforms and across runs.
pub(crate) fn worktree_roots(repo_root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for parent in [
        repo_root.join(".worktrees"),
        repo_root.join(".claude").join("worktrees"),
    ] {
        let Ok(entries) = fs::read_dir(&parent) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        children.sort();
        roots.extend(children);
    }
    roots
}

pub(crate) fn scan_worktrees(
    repo_root: &Path,
    workflow_rel: &Path,
    allowed_statuses: &[String],
    id_style: Option<&str>,
) -> Result<(Vec<Entity>, Vec<EntityParseError>), ParseError> {
    let mut all_items = Vec::new();
    let mut all_errors: Vec<EntityParseError> = Vec::new();
    for wt_root in worktree_roots(repo_root) {
        let candidate = wt_root.join(workflow_rel);
        if !candidate.is_dir() {
            continue;
        }
        let (items, errors) = load_worktree_items(&candidate, allowed_statuses, id_style)?;
        all_items.extend(items);
        all_errors.extend(errors);
    }
    Ok((all_items, all_errors))
}

fn load_worktree_items(
    workflow_dir: &Path,
    allowed_statuses: &[String],
    id_style: Option<&str>,
) -> Result<(Vec<Entity>, Vec<EntityParseError>), ParseError> {
    let item_paths = collect_worktree_item_paths(workflow_dir);
    let mut items = Vec::with_capacity(item_paths.len());
    let mut errors: Vec<EntityParseError> = Vec::new();
    for item_path in item_paths {
        match parse_work_item(&item_path, allowed_statuses, id_style) {
            Ok(item) => items.push(item),
            Err(err) if err.is_per_entity_parse_failure() => {
                errors.push(entity_parse_error_from(&item_path, &err));
            }
            Err(err) => return Err(err),
        }
    }
    Ok((items, errors))
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
/// Uses SHA-1 digest (not string equality on body) for content comparison.
pub(crate) fn merge_worktree_items(
    main_items: Vec<Entity>,
    worktree_items: Vec<Entity>,
) -> Vec<Entity> {
    if worktree_items.is_empty() {
        return main_items;
    }
    let mut index: HashMap<std::ffi::OsString, Entity> = main_items
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
            // Worktree-only item: tag the row so the UI can mark it.
            let wt_path = wt_item.path.clone();
            let mut tagged = wt_item;
            tagged.worktree_source = Some(wt_path);
            tagged.main_body = None;
            index.insert(slug, tagged);
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

fn merged_worktree_item(main_item: &Entity, wt_item: Entity) -> Option<Entity> {
    match (content_hash(&wt_item.path), content_hash(&main_item.path)) {
        (Some(wt_hash), Some(main_hash)) if wt_hash == main_hash => None,
        (None, Some(_)) => None,
        (Some(_), Some(_)) => Some(merge_main_frontmatter_with_worktree_body(
            main_item, wt_item,
        )),
        (Some(_), None) | (None, None) => {
            // Cannot read main copy; fall back to worktree-only treatment.
            let wt_path = wt_item.path.clone();
            let mut tagged = wt_item;
            tagged.worktree_source = Some(wt_path);
            tagged.main_body = None;
            Some(tagged)
        }
    }
}

fn content_hash(path: &Path) -> Option<[u8; 20]> {
    fs::read(path).map(|bytes| Sha1::digest(&bytes).into()).ok()
}

fn merge_main_frontmatter_with_worktree_body(main_item: &Entity, wt_item: Entity) -> Entity {
    let wt_path = wt_item.path.clone();
    Entity {
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
        worktree_source: Some(wt_path),
        main_body: Some(main_item.body.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_roots_orders_deterministically_with_claude_after_plain() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = dir.path();
        // Create children in a non-sorted order to avoid relying on insertion
        // order. fs::read_dir gives platform-dependent order; the function
        // must sort.
        for name in ["c-task", "a-task", "b-task"] {
            fs::create_dir_all(repo.join(".worktrees").join(name)).expect("mkdir worktrees child");
        }
        for name in ["zeta", "alpha", "mu"] {
            fs::create_dir_all(repo.join(".claude").join("worktrees").join(name))
                .expect("mkdir claude worktrees child");
        }

        let first = worktree_roots(repo);
        let second = worktree_roots(repo);
        assert_eq!(first, second, "worktree_roots must be deterministic");

        let expected: Vec<PathBuf> = vec![
            repo.join(".worktrees/a-task"),
            repo.join(".worktrees/b-task"),
            repo.join(".worktrees/c-task"),
            repo.join(".claude/worktrees/alpha"),
            repo.join(".claude/worktrees/mu"),
            repo.join(".claude/worktrees/zeta"),
        ];
        assert_eq!(first, expected);
    }
}
