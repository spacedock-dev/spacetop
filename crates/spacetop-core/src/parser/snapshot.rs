use std::path::{Path, PathBuf};

use crate::domain::{EntityParseError, WorkflowSnapshot};

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
    // README parse stays on the definition dir (`path`); entity/archive scans
    // read from the resolved entity dir, which differs only for split-root
    // workflows declaring `state:`.
    let entity_dir = resolve_entity_dir(path, definition.state.as_deref());
    let item_paths = collect_active_item_paths(&entity_dir)?;
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

    // The worktree scan is anchored on the definition dir's repo-relative path:
    // split-root state entities are not mirrored under `.worktrees/<task>/<workflow>`,
    // so this is effectively a no-op for them. The merge base, however, points at
    // the entity dir so `archived_slug_exists` consults the resolved `_archive/`.
    let (worktree_items, worktree_parse_errors) = match path.strip_prefix(repo_root) {
        Ok(workflow_rel) => scan_worktrees(repo_root, workflow_rel, &allowed_statuses, id_style)?,
        Err(_) => (Vec::new(), Vec::new()),
    };
    parse_errors.extend(worktree_parse_errors);
    let items = merge_worktree_items(items, worktree_items, &entity_dir);

    Ok(WorkflowSnapshot {
        definition,
        items,
        parse_errors,
    })
}

/// Resolve the entity directory from a definition directory and its README
/// `state:` declaration. The entity directory is where active `*.md` entities
/// and `_archive/` are read; the definition directory is where `README.md`
/// lives (and what discovery returns).
///
/// - A relative `state:` (e.g. `.spacedock-state`) → `definition_dir.join(state)`.
/// - `None`, empty, or the sentinel `$inline` → the definition directory
///   itself (single-root, unchanged behavior).
///
/// Resolution is always relative to the definition directory, never the repo
/// root or cwd. An absolute `state:` is out of scope for this task; it would
/// fall through to `join`, which discards the definition prefix — fixtures do
/// not exercise it and it is not special-cased.
pub(crate) fn resolve_entity_dir(definition_dir: &Path, state: Option<&str>) -> PathBuf {
    match state.map(str::trim) {
        None | Some("") | Some("$inline") => definition_dir.to_path_buf(),
        Some(rel) => definition_dir.join(rel),
    }
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
    // A declared split-root state checkout may be absent (e.g. when reading from
    // a code worktree that does not carry the shared state checkout). Treat a
    // missing entity directory as "no active entities yet", mirroring how a
    // missing `_archive/` is tolerated, rather than failing the whole load. The
    // single-root case never hits this: a parsed README guarantees its own dir.
    if !workflow_dir.is_dir() {
        return Ok(Vec::new());
    }
    let entries = read_directory(workflow_dir)?;
    let mut item_paths = Vec::new();
    for entry in entries {
        let entry_path = entry.path();
        if is_readme_path(&entry_path) {
            continue;
        }
        if is_markdown_path(&entry_path) {
            item_paths.push(entry_path);
            continue;
        }
        if entry_path.is_dir() {
            let index_path = entry_path.join("index.md");
            if index_path.is_file() {
                item_paths.push(index_path);
            }
        }
    }
    item_paths.sort();
    Ok(item_paths)
}

#[cfg(test)]
mod tests {
    use super::resolve_entity_dir;
    use std::path::Path;

    #[test]
    fn relative_state_joins_definition_dir() {
        let def = Path::new("/repo/docs/wf");
        assert_eq!(
            resolve_entity_dir(def, Some(".spacedock-state")),
            def.join(".spacedock-state")
        );
    }

    #[test]
    fn inline_empty_and_absent_state_resolve_to_definition_dir() {
        let def = Path::new("/repo/docs/wf");
        for state in [None, Some(""), Some("  "), Some("$inline")] {
            assert_eq!(
                resolve_entity_dir(def, state),
                def.to_path_buf(),
                "state {state:?} should keep entity dir == definition dir"
            );
        }
    }
}
