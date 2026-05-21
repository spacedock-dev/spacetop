use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use spacetop::cli::Cli;
use spacetop::{decide_app, DecideOutcome};
use tempfile::tempdir;

/// Write a minimal Spacedock workflow: a README with the `commissioned-by`
/// signal plus a `stages:` block so `App::load` succeeds against it.
fn write_workflow(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    let readme = r#"---
commissioned-by: spacedock@0.10.1
entity-type: task
entity-label: task
entity-label-plural: tasks
id-style: sequential
stages:
  defaults:
    worktree: false
  states:
    - name: plan
      initial: true
    - name: done
      terminal: true
---

# Fixture Workflow

body
"#;
    fs::write(dir.join("README.md"), readme).unwrap();
}

fn cli_with(workflow_dir: Option<PathBuf>) -> Cli {
    Cli { workflow_dir }
}

#[test]
fn multi_workflow_fixture_opens_first_workflow_dashboard() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join(".git")).unwrap();
    write_workflow(&root.join("docs/alpha"));
    write_workflow(&root.join("docs/beta"));

    let outcome = decide_app(&cli_with(None), root).unwrap();
    match outcome {
        DecideOutcome::Overview(app) => {
            let session = app.as_session().expect("overview session");
            assert_eq!(session.len(), 2);
            assert_eq!(session.active_index(), 0);
            assert_eq!(
                app.workflow_dir(),
                fs::canonicalize(root.join("docs/alpha")).unwrap().as_path()
            );
        }
        other => panic!("expected Overview, got {other:?}"),
    }
}

#[test]
fn single_workflow_fixture_yields_overview_variant() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join(".git")).unwrap();
    write_workflow(&root.join("docs/only"));

    let outcome = decide_app(&cli_with(None), root).unwrap();
    match outcome {
        DecideOutcome::Overview(app) => {
            let only_canonical = fs::canonicalize(root.join("docs/only")).unwrap();
            assert_eq!(app.workflow_dir(), only_canonical.as_path());
        }
        other => panic!("expected Overview, got {other:?}"),
    }
}

#[test]
fn zero_workflow_fixture_yields_error_variant_with_scan_root() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join(".git")).unwrap();

    let outcome = decide_app(&cli_with(None), root).unwrap();
    match outcome {
        DecideOutcome::ZeroWorkflows { scan_root } => {
            assert_eq!(
                fs::canonicalize(&scan_root).unwrap(),
                fs::canonicalize(root).unwrap()
            );
        }
        other => panic!("expected ZeroWorkflows, got {other:?}"),
    }
}

#[test]
fn worktrees_excluded_from_decide_app_discovery() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join(".git")).unwrap();
    // One real workflow
    write_workflow(&root.join("docs/real"));
    // Same workflow mirrored inside a worktree clone — must not duplicate the entry
    write_workflow(&root.join(".worktrees/some-task/docs/real"));

    let outcome = decide_app(&cli_with(None), root).unwrap();
    match outcome {
        DecideOutcome::Overview(app) => {
            let session = app.as_session().expect("overview session");
            assert_eq!(
                session.len(),
                1,
                "worktree clone must not inflate workflow count"
            );
        }
        other => panic!("expected Overview, got {other:?}"),
    }
}

#[test]
fn explicit_w_bypasses_discovery_even_when_other_workflows_exist() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join(".git")).unwrap();
    write_workflow(&root.join("docs/alpha"));
    write_workflow(&root.join("docs/beta"));

    // Point `-w` at alpha explicitly. Discovery must NOT run.
    let explicit = root.join("docs/alpha");
    let outcome = decide_app(&cli_with(Some(explicit.clone())), root).unwrap();
    match outcome {
        DecideOutcome::Overview(app) => {
            assert_eq!(app.workflow_dir(), explicit.as_path());
        }
        other => panic!("expected Overview from explicit -w, got {other:?}"),
    }
}

#[test]
fn explicit_w_repo_root_falls_back_to_discovery_within_that_root() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join("README.md"), "# Plain repo root\n").unwrap();
    write_workflow(&root.join("docs/alpha"));
    write_workflow(&root.join("docs/beta"));

    let outcome = decide_app(&cli_with(Some(root.to_path_buf())), root).unwrap();
    match outcome {
        DecideOutcome::Overview(app) => {
            let session = app.as_session().expect("overview session");
            assert_eq!(session.len(), 2);
            assert_eq!(
                session.scan_root().expect("scan root"),
                fs::canonicalize(root).unwrap().as_path()
            );
            assert_eq!(
                app.workflow_dir(),
                fs::canonicalize(root.join("docs/alpha")).unwrap().as_path()
            );
        }
        other => panic!("expected discovered overview from explicit repo root, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn explicit_w_repo_root_symlink_uses_canonical_scan_root() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join("README.md"), "# Plain repo root\n").unwrap();
    write_workflow(&root.join("docs/alpha"));
    write_workflow(&root.join("docs/beta"));

    let symlink_root = root.join("repo-link");
    symlink(root, &symlink_root).unwrap();

    let outcome = decide_app(&cli_with(Some(symlink_root)), root).unwrap();
    match outcome {
        DecideOutcome::Overview(app) => {
            let session = app.as_session().expect("overview session");
            assert_eq!(
                session.scan_root().expect("scan root"),
                fs::canonicalize(root).unwrap().as_path()
            );
        }
        other => panic!("expected discovered overview from symlinked repo root, got {other:?}"),
    }
}

#[test]
fn explicit_w_repo_root_single_workflow_opens_that_workflow() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join("README.md"), "# Plain repo root\n").unwrap();
    write_workflow(&root.join("docs/only"));

    let outcome = decide_app(&cli_with(Some(root.to_path_buf())), root).unwrap();
    match outcome {
        DecideOutcome::Overview(app) => {
            assert_eq!(
                app.workflow_dir(),
                fs::canonicalize(root.join("docs/only")).unwrap().as_path()
            );
        }
        other => {
            panic!("expected single discovered workflow from explicit repo root, got {other:?}")
        }
    }
}
