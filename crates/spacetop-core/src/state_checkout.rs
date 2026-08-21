//! Read-only classification of split-root workflow state checkouts.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::domain::{StateCheckoutDisposition, WorkflowStorage};
use crate::git::GitRunner;

/// Classify the README storage declaration and, for a supported split-root,
/// probe the checkout disposition without consulting the network or mutating
/// Git state.
pub fn classify_storage<R: GitRunner>(
    runner: &R,
    definition_dir: &Path,
    state: Option<&str>,
    state_branch: Option<&str>,
) -> WorkflowStorage {
    let Some(entity_dir) = split_root_entity_dir(definition_dir, state) else {
        return WorkflowStorage::SingleRoot;
    };
    let expected_branch = expected_state_branch(definition_dir, state_branch);
    let disposition = probe_disposition(runner, definition_dir, &entity_dir, &expected_branch);
    WorkflowStorage::SplitRoot {
        entity_dir,
        expected_branch,
        disposition,
    }
}

/// Resolve only supported contained relative split-root declarations.
pub fn split_root_entity_dir(definition_dir: &Path, state: Option<&str>) -> Option<PathBuf> {
    let rel = match state.map(str::trim) {
        None | Some("") | Some("$inline") => return None,
        Some(rel) => Path::new(rel),
    };
    if rel.is_absolute()
        || rel
            .components()
            .any(|component| component == Component::ParentDir)
    {
        return None;
    }
    Some(definition_dir.join(rel))
}

pub fn expected_state_branch(definition_dir: &Path, state_branch: Option<&str>) -> String {
    if let Some(branch) = state_branch
        .map(str::trim)
        .filter(|branch| !branch.is_empty())
    {
        return branch.to_string();
    }
    let workflow_name = definition_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workflow");
    format!("spacedock-state/{workflow_name}")
}

fn probe_disposition<R: GitRunner>(
    runner: &R,
    definition_dir: &Path,
    entity_dir: &Path,
    expected_branch: &str,
) -> StateCheckoutDisposition {
    if !entity_dir.is_dir() {
        return StateCheckoutDisposition::Missing;
    }

    let definition_top = match fs::canonicalize(definition_dir) {
        Ok(path) => path,
        Err(error) => {
            return probe_failed(format!(
                "cannot resolve workflow definition directory: {error}"
            ));
        }
    };
    let expected_top = match fs::canonicalize(entity_dir) {
        Ok(path) => path,
        Err(error) => return probe_failed(format!("cannot resolve state directory: {error}")),
    };
    if !expected_top.starts_with(&definition_top) {
        return probe_failed(format!(
            "state directory resolves outside workflow definition directory: {}",
            expected_top.display()
        ));
    }
    let top = match runner.run(entity_dir, &["rev-parse", "--show-toplevel"]) {
        Ok(result) if result.status.success() => result.stdout.trim().to_string(),
        Ok(result) => return probe_failed(git_failure(&result.stderr, "not a Git checkout")),
        Err(error) => return probe_failed(format!("Git probe failed: {error}")),
    };
    if top.is_empty() {
        return probe_failed("Git reported an empty checkout root".to_string());
    }
    let actual_top = match fs::canonicalize(&top) {
        Ok(path) => path,
        Err(error) => return probe_failed(format!("cannot resolve Git checkout root: {error}")),
    };
    if actual_top != expected_top {
        return probe_failed("state directory belongs to a parent Git checkout".to_string());
    }

    match runner.run(entity_dir, &["symbolic-ref", "--quiet", "--short", "HEAD"]) {
        Ok(result) if result.status.success() => {
            let actual_branch = result.stdout.trim().to_string();
            if actual_branch.is_empty() {
                probe_failed("Git reported an empty branch name".to_string())
            } else if actual_branch == expected_branch {
                StateCheckoutDisposition::Attached
            } else {
                StateCheckoutDisposition::WrongBranch { actual_branch }
            }
        }
        Ok(result) if result.status.code() == Some(1) => StateCheckoutDisposition::Detached,
        Ok(result) => probe_failed(git_failure(&result.stderr, "branch probe failed")),
        Err(error) => probe_failed(format!("Git branch probe failed: {error}")),
    }
}

fn git_failure(stderr: &str, fallback: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn probe_failed(reason: String) -> StateCheckoutDisposition {
    StateCheckoutDisposition::ProbeFailed { reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::{err, ok, RecordingGitRunner};
    use tempfile::tempdir;

    #[test]
    fn backend_classifier_keeps_inline_and_unsupported_paths_single_root() {
        let runner = RecordingGitRunner::new(Vec::new());
        let root = Path::new("/repo/docs/demo");
        for state in [
            None,
            Some(""),
            Some("$inline"),
            Some("../escape"),
            Some("/tmp/x"),
        ] {
            assert_eq!(
                classify_storage(&runner, root, state, None),
                WorkflowStorage::SingleRoot
            );
        }
        assert!(runner.calls().is_empty(), "single-root must not probe Git");
    }

    #[test]
    fn missing_split_root_stays_typed_split_root_without_git_probe() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("demo");
        fs::create_dir(&root).expect("definition dir");
        let runner = RecordingGitRunner::new(Vec::new());
        assert_eq!(
            classify_storage(&runner, &root, Some("state"), None),
            WorkflowStorage::SplitRoot {
                entity_dir: root.join("state"),
                expected_branch: "spacedock-state/demo".to_string(),
                disposition: StateCheckoutDisposition::Missing,
            }
        );
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn named_branches_classify_as_attached_or_wrong_branch() {
        let temp = tempdir().expect("tempdir");
        let state = temp.path().join("state");
        fs::create_dir(&state).expect("state dir");
        let top = fs::canonicalize(&state).expect("canonical state");
        let attached = RecordingGitRunner::new(vec![
            ok(&format!("{}\n", top.display())),
            ok("spacedock-state/demo\n"),
        ]);
        assert_eq!(
            probe_disposition(&attached, temp.path(), &state, "spacedock-state/demo"),
            StateCheckoutDisposition::Attached
        );

        let wrong = RecordingGitRunner::new(vec![
            ok(&format!("{}\n", top.display())),
            ok("wrong-state\n"),
        ]);
        assert_eq!(
            probe_disposition(&wrong, temp.path(), &state, "spacedock-state/demo"),
            StateCheckoutDisposition::WrongBranch {
                actual_branch: "wrong-state".to_string()
            }
        );
    }

    #[test]
    fn detached_and_probe_failures_are_distinct() {
        let temp = tempdir().expect("tempdir");
        let state = temp.path().join("state");
        fs::create_dir(&state).expect("state dir");
        let top = fs::canonicalize(&state).expect("canonical state");
        let detached =
            RecordingGitRunner::new(vec![ok(&format!("{}\n", top.display())), err(1, "")]);
        assert_eq!(
            probe_disposition(&detached, temp.path(), &state, "spacedock-state/demo"),
            StateCheckoutDisposition::Detached
        );

        let failed = RecordingGitRunner::new(vec![err(128, "fatal: not a git repository\n")]);
        assert_eq!(
            probe_disposition(&failed, temp.path(), &state, "spacedock-state/demo"),
            StateCheckoutDisposition::ProbeFailed {
                reason: "fatal: not a git repository".to_string()
            }
        );
    }

    #[test]
    fn parent_repository_fallthrough_fails_closed() {
        let temp = tempdir().expect("tempdir");
        let state = temp.path().join("state");
        fs::create_dir(&state).expect("state dir");
        let parent = fs::canonicalize(temp.path()).expect("canonical parent");
        let runner = RecordingGitRunner::new(vec![ok(&format!("{}\n", parent.display()))]);
        assert_eq!(
            probe_disposition(&runner, temp.path(), &state, "spacedock-state/demo"),
            StateCheckoutDisposition::ProbeFailed {
                reason: "state directory belongs to a parent Git checkout".to_string()
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn external_state_symlink_fails_before_git_probe() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tempdir");
        let definition = temp.path().join("demo");
        let external = temp.path().join("external-state");
        fs::create_dir(&definition).expect("definition dir");
        fs::create_dir(&external).expect("external state dir");
        symlink(&external, definition.join(".spacedock-state")).expect("state symlink");
        let runner = RecordingGitRunner::new(Vec::new());

        assert!(matches!(
            classify_storage(
                &runner,
                &definition,
                Some(".spacedock-state"),
                None
            ),
            WorkflowStorage::SplitRoot {
                disposition: StateCheckoutDisposition::ProbeFailed { ref reason },
                ..
            } if reason.contains("resolves outside workflow definition directory")
        ));
        assert!(
            runner.calls().is_empty(),
            "escaped state path must be rejected before any Git probe"
        );
    }
}
