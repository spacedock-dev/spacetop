use std::path::{Path, PathBuf};

use anyhow::Context;
use spacetop_core::discovery;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadlessWorkflow {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub workflow_rel: String,
}

pub fn resolve_workflow_arg(
    workflow_dir: Option<PathBuf>,
    cwd: &Path,
) -> anyhow::Result<HeadlessWorkflow> {
    let requested = match workflow_dir {
        Some(path) if path.is_absolute() => path,
        Some(path) => cwd.join(path),
        None => cwd.to_path_buf(),
    }
    .canonicalize()
    .with_context(|| "failed to resolve workflow path")?;

    let workflows = discovery::discover_workflows(&requested)
        .with_context(|| format!("failed to scan {}", requested.display()))?;
    if workflows.len() != 1 {
        anyhow::bail!(
            "headless command requires exactly one workflow; pass --workflow-dir <path>"
        );
    }

    let workflow_dir = workflows[0].root.clone();
    let repo_root = discovery::resolve_scan_root(&workflow_dir);
    let workflow_rel = workflow_dir
        .strip_prefix(&repo_root)
        .map(path_to_git_rel)
        .unwrap_or_else(|_| workflow_dir.to_string_lossy().into_owned());

    Ok(HeadlessWorkflow {
        workflow_dir,
        repo_root,
        workflow_rel,
    })
}

fn path_to_git_rel(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn explicit_workflow_path_canonicalizes_and_resolves_direct_workflow() {
        let repo = fixture_repo_with_one_workflow();
        let path = repo.path().join("docs/workflow");

        let resolved = resolve_workflow_arg(Some(path.clone()), repo.path()).expect("resolve");

        assert_eq!(resolved.workflow_dir, path.canonicalize().expect("canonical"));
        assert_eq!(resolved.repo_root, repo.path().canonicalize().expect("repo"));
        assert_eq!(resolved.workflow_rel, "docs/workflow");
    }

    #[test]
    fn explicit_scan_root_must_discover_exactly_one_workflow() {
        let repo = fixture_repo_with_one_workflow();

        let resolved =
            resolve_workflow_arg(Some(repo.path().to_path_buf()), repo.path()).expect("resolve");

        assert!(resolved.workflow_dir.ends_with("docs/workflow"));
        assert_eq!(resolved.repo_root, repo.path().canonicalize().expect("repo"));
        assert_eq!(resolved.workflow_rel, "docs/workflow");
    }

    #[test]
    fn omitted_path_rejects_zero_or_multiple_workflows() {
        let empty = tempfile::tempdir().expect("tempdir");
        let err = resolve_workflow_arg(None, empty.path())
            .unwrap_err()
            .to_string();
        assert!(err.contains("headless command requires exactly one workflow"));

        let repo = fixture_repo_with_two_workflows();
        let err = resolve_workflow_arg(None, repo.path())
            .unwrap_err()
            .to_string();
        assert!(err.contains("headless command requires exactly one workflow"));
    }

    fn fixture_repo_with_one_workflow() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(repo.path().join(".git")).expect("git dir");
        write_workflow(&repo.path().join("docs/workflow"));
        repo
    }

    fn fixture_repo_with_two_workflows() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(repo.path().join(".git")).expect("git dir");
        write_workflow(&repo.path().join("docs/alpha"));
        write_workflow(&repo.path().join("docs/beta"));
        repo
    }

    fn write_workflow(dir: &Path) {
        std::fs::create_dir_all(dir).expect("workflow dir");
        std::fs::write(
            dir.join("README.md"),
            "---\ncommissioned-by: spacedock@test\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: done\n      terminal: true\n---\n\n# Workflow\n",
        )
        .expect("write readme");
    }
}
