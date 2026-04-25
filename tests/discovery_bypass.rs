use std::fs;
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
