use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

use crate::parser::{split_frontmatter, SplitFrontmatter};

/// A Spacedock workflow directory discovered by [`discover_workflows`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredWorkflow {
    /// Canonical path to the workflow directory.
    pub root: PathBuf,
    /// Best-effort title read from the README's first `#` heading. `None` if
    /// the heading could not be read for any reason.
    pub title: Option<String>,
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("discovery IO error: {0}")]
    Io(#[from] io::Error),
}

/// Directory names pruned from the discovery walk at any depth.
pub const PRUNED_DIR_NAMES: &[&str] = &[
    ".git",
    ".worktrees",
    "node_modules",
    "vendor",
    "dist",
    "build",
    "__pycache__",
    "tests",
];

/// A directory's `README.md` identifies the dir as a Spacedock workflow when
/// the YAML frontmatter has a `commissioned-by:` value starting with this
/// literal prefix. Matches `spacedock --discover`'s rule verbatim.
pub const SPACEDOCK_COMMISSION_PREFIX: &str = "spacedock@";

/// Resolve the scan root: walk upward from `cwd` looking for a `.git` entry
/// (directory or file). Falls back to `cwd` when no git root is found.
pub fn resolve_scan_root(cwd: &Path) -> PathBuf {
    let mut current: Option<&Path> = Some(cwd);
    while let Some(dir) = current {
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
        current = dir.parent();
    }
    cwd.to_path_buf()
}

/// Walk `root` looking for Spacedock workflow directories. Symlinks are
/// followed with cycle protection (visited real paths are tracked). Candidate
/// READMEs with unparseable or non-matching frontmatter are silently ignored
/// (they are "not workflows", not errors).
pub fn discover_workflows(root: &Path) -> Result<Vec<DiscoveredWorkflow>, DiscoveryError> {
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut out_paths: HashSet<PathBuf> = HashSet::new();
    let mut out: Vec<DiscoveredWorkflow> = Vec::new();

    let walker = WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_pruned(entry));

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                // Loop errors (symlink cycles via follow_links) are reported by
                // walkdir; treat as soft: skip and continue.
                if err.loop_ancestor().is_some() {
                    continue;
                }
                // Bubble up real IO errors so callers see them.
                let depth = err.depth();
                if let Some(io_err) = err.into_io_error() {
                    // Root path does not exist: treat as empty, not a hard error.
                    // This routes through the ZeroWorkflows branch in lib.rs.
                    if io_err.kind() == io::ErrorKind::NotFound && depth == 0 {
                        return Ok(vec![]);
                    }
                    // Sub-entry disappeared mid-walk (broken symlink, concurrent delete)
                    // — skip and continue rather than aborting the whole scan.
                    if io_err.kind() == io::ErrorKind::NotFound {
                        continue;
                    }
                    return Err(DiscoveryError::Io(io_err));
                }
                continue;
            }
        };

        if !entry.file_type().is_dir() {
            continue;
        }

        // Cycle protection: dedupe visited canonical dirs.
        let canonical = match fs::canonicalize(entry.path()) {
            Ok(path) => path,
            Err(_) => continue,
        };
        if !visited.insert(canonical.clone()) {
            continue;
        }

        let readme = entry.path().join("README.md");
        if !readme.is_file() {
            continue;
        }

        if read_commission_marker(&readme)
            .map(|marker| marker.starts_with(SPACEDOCK_COMMISSION_PREFIX))
            .unwrap_or(false)
            && out_paths.insert(canonical.clone())
        {
            out.push(DiscoveredWorkflow {
                title: read_title(&readme),
                root: canonical,
            });
        }
    }

    out.sort_by(|a, b| a.root.cmp(&b.root));
    Ok(out)
}

fn is_pruned(entry: &DirEntry) -> bool {
    // Don't prune the root itself based on name.
    if entry.depth() == 0 {
        return false;
    }
    if !entry.file_type().is_dir() {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .map(|name| PRUNED_DIR_NAMES.contains(&name))
        .unwrap_or(false)
}

#[derive(Debug, Deserialize)]
struct CommissionFrontmatter {
    #[serde(rename = "commissioned-by")]
    commissioned_by: Option<String>,
}

fn read_commission_marker(readme: &Path) -> Option<String> {
    let contents = fs::read_to_string(readme).ok()?;
    let SplitFrontmatter::Ok { frontmatter, .. } = split_frontmatter(&contents)? else {
        return None;
    };
    let parsed: CommissionFrontmatter = serde_yaml::from_str(frontmatter).ok()?;
    parsed.commissioned_by
}

fn read_title(readme: &Path) -> Option<String> {
    let contents = fs::read_to_string(readme).ok()?;
    let body = match split_frontmatter(&contents)? {
        SplitFrontmatter::Ok { body, .. } => body,
        SplitFrontmatter::Unterminated => return None,
    };
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_workflow_readme(dir: &Path, title: &str) {
        fs::create_dir_all(dir).expect("create workflow dir");
        let readme = format!("---\ncommissioned-by: spacedock@0.10.1\n---\n\n# {title}\n\nbody\n");
        fs::write(dir.join("README.md"), readme).expect("write readme");
    }

    #[test]
    fn nonexistent_root_returns_empty_not_error() {
        let tmp = tempdir().unwrap();
        let missing = tmp.path().join("definitely-does-not-exist");

        let result = discover_workflows(&missing);
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn discovers_multiple_workflows_in_fixture_tree() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        write_workflow_readme(&root.join("docs/alpha"), "Alpha");
        write_workflow_readme(&root.join("pipelines/beta"), "Beta");

        let found = discover_workflows(root).unwrap();
        assert_eq!(found.len(), 2, "expected two workflows, got {found:?}");
        let titles: Vec<_> = found.iter().map(|w| w.title.as_deref()).collect();
        assert!(titles.contains(&Some("Alpha")));
        assert!(titles.contains(&Some("Beta")));
    }

    #[test]
    fn discovers_single_workflow() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        write_workflow_readme(&root.join("docs/only"), "Only");

        let found = discover_workflows(root).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title.as_deref(), Some("Only"));
    }

    #[test]
    fn discovers_zero_workflows() {
        let tmp = tempdir().unwrap();
        let found = discover_workflows(tmp.path()).unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn prunes_directory_names_at_any_depth() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        write_workflow_readme(&root.join("node_modules/foo"), "Hidden");
        write_workflow_readme(&root.join("docs/real"), "Real");

        let found = discover_workflows(root).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title.as_deref(), Some("Real"));
    }

    #[test]
    fn non_spacedock_readmes_are_ignored() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let other = root.join("docs/other");
        fs::create_dir_all(&other).unwrap();
        fs::write(
            other.join("README.md"),
            "---\ncommissioned-by: other@1.0\n---\n\n# Other\n",
        )
        .unwrap();

        let plain = root.join("docs/plain");
        fs::create_dir_all(&plain).unwrap();
        fs::write(plain.join("README.md"), "# Plain\n").unwrap();

        let found = discover_workflows(root).unwrap();
        assert!(found.is_empty(), "expected none, got {found:?}");
    }

    #[test]
    fn resolve_scan_root_walks_up_to_dotgit() {
        let tmp = tempdir().unwrap();
        let top = tmp.path();
        fs::create_dir_all(top.join(".git")).unwrap();
        let nested = top.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();

        let resolved = resolve_scan_root(&nested);
        // canonicalize both sides for comparison (tempdir can be symlinked on macOS)
        assert_eq!(
            fs::canonicalize(&resolved).unwrap(),
            fs::canonicalize(top).unwrap()
        );
    }

    #[test]
    fn resolve_scan_root_falls_back_to_cwd_without_dotgit() {
        let tmp = tempdir().unwrap();
        let resolved = resolve_scan_root(tmp.path());
        assert_eq!(
            fs::canonicalize(&resolved).unwrap(),
            fs::canonicalize(tmp.path()).unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn dedupes_symlinked_duplicate_by_realpath() {
        use std::os::unix::fs::symlink;
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        write_workflow_readme(&root.join("docs/real"), "Real");
        symlink(root.join("docs/real"), root.join("docs/alias")).unwrap();

        let found = discover_workflows(root).unwrap();
        assert_eq!(found.len(), 1, "expected dedup, got {found:?}");
        let canonical_real = fs::canonicalize(root.join("docs/real")).unwrap();
        assert_eq!(found[0].root, canonical_real);
    }

    #[test]
    fn worktrees_subdir_is_excluded_from_discovery() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        // Real workflow at docs/real
        write_workflow_readme(&root.join("docs/real"), "Real");
        // Same workflow mirrored inside a worktree clone — must be skipped
        write_workflow_readme(&root.join(".worktrees/some-task/docs/real"), "Real");

        let found = discover_workflows(root).unwrap();
        assert_eq!(found.len(), 1, "expected 1 workflow, got {found:?}");
        // The returned path must not contain `.worktrees` as a component
        let has_worktrees = found[0]
            .root
            .components()
            .any(|c| c.as_os_str() == ".worktrees");
        assert!(!has_worktrees, "result path must not be inside .worktrees");
    }

    #[test]
    fn worktrees_clone_does_not_inflate_workflow_count() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        // Two real workflows
        write_workflow_readme(&root.join("docs/alpha"), "Alpha");
        write_workflow_readme(&root.join("docs/beta"), "Beta");
        // Worktree clone with the same two workflows
        write_workflow_readme(&root.join(".worktrees/task-1/docs/alpha"), "Alpha");
        write_workflow_readme(&root.join(".worktrees/task-1/docs/beta"), "Beta");

        let found = discover_workflows(root).unwrap();
        assert_eq!(found.len(), 2, "expected 2 workflows, got {found:?}");
    }

    #[cfg(unix)]
    #[test]
    fn broken_symlink_in_subtree_is_skipped_not_fatal() {
        use std::os::unix::fs::symlink;
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        // A real workflow so we can confirm the walk completes and finds it.
        write_workflow_readme(&root.join("docs/real"), "Real");
        // Broken symlink: target does not exist.
        symlink(root.join("nonexistent-target"), root.join("docs/broken-link")).unwrap();

        let result = discover_workflows(root);
        assert!(result.is_ok(), "broken symlink must not be fatal, got {result:?}");
        let found = result.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title.as_deref(), Some("Real"));
    }

    #[cfg(unix)]
    #[test]
    fn permission_denied_on_root_surfaces_as_error() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("locked");
        fs::create_dir_all(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();

        let result = discover_workflows(&root);
        // Restore permissions before asserting so tempdir cleanup succeeds.
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(result.is_err(), "PermissionDenied on root must return Err, got {result:?}");
    }

    #[cfg(unix)]
    #[test]
    fn handles_symlink_cycle_without_infinite_loop() {
        use std::os::unix::fs::symlink;
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let a = root.join("a");
        let b = root.join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        // a/to_b -> b, b/to_a -> a
        symlink(&b, a.join("to_b")).unwrap();
        symlink(&a, b.join("to_a")).unwrap();
        write_workflow_readme(&a.join("workflow"), "Cycle");

        let found = discover_workflows(root).unwrap();
        // Accept "at least one" (the canonical a/workflow); cycle doesn't matter
        // as long as we return without hanging.
        assert!(!found.is_empty());
    }
}
