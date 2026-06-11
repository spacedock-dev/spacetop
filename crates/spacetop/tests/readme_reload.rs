//! Integration tests for live README reload (045).
//!
//! These drive `App::reload_with_rediscovery` directly against tempdir
//! fixtures so the live `notify` backend never races the assertions; the
//! watcher itself is covered by `tests/watcher_fs.rs`.

use std::fs;
use std::path::{Path, PathBuf};

use spacetop::app::{App, OverviewSession, OverviewState};
use spacetop_core::discovery::{discover_workflows, DiscoveredWorkflow};
use tempfile::tempdir;

const COMMISSION_HEADER: &str = "commissioned-by: spacedock@0.10.1\n";

fn write_workflow_readme(dir: &Path, stages: &[&str]) {
    fs::create_dir_all(dir).expect("create workflow dir");
    let mut readme = String::from("---\n");
    readme.push_str(COMMISSION_HEADER);
    readme.push_str("stages:\n  states:\n");
    for (i, name) in stages.iter().enumerate() {
        readme.push_str(&format!("    - name: {name}\n"));
        if i == 0 {
            readme.push_str("      initial: true\n");
        }
        if i == stages.len() - 1 {
            readme.push_str("      terminal: true\n");
        }
    }
    readme.push_str("---\n\n# Workflow\n\nbody\n");
    fs::write(dir.join("README.md"), readme).expect("write README");
}

fn build_session(scan_root: &Path) -> App {
    let workflows = discover_workflows(scan_root).expect("discover");
    assert!(
        !workflows.is_empty(),
        "fixture must have at least one workflow"
    );
    let first = workflows[0].root.clone();
    let initial = OverviewState::load(first).expect("load initial");
    let session = OverviewSession::from_discovery(scan_root.to_path_buf(), workflows, 0, initial);
    App::from_session(session)
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonicalize")
}

fn discovery_roots(workflows: &[DiscoveredWorkflow]) -> Vec<PathBuf> {
    workflows.iter().map(|w| w.root.clone()).collect()
}

/// AC-1: editing a watched workflow's README re-parses the in-memory
/// definition. We don't wait on the live watcher signal; we drive the
/// reload path directly to prove the parser+state plumbing is correct.
#[test]
fn readme_edit_reparses_definition_live() {
    let tmp = tempdir().unwrap();
    let scan_root = tmp.path();
    write_workflow_readme(&scan_root.join("docs/alpha"), &["design", "done"]);

    let mut app = build_session(scan_root);
    let initial_stages: Vec<String> = app
        .snapshot()
        .definition
        .stages
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(
        initial_stages,
        vec!["design".to_string(), "done".to_string()]
    );

    // Overwrite the README with a three-stage definition.
    write_workflow_readme(&scan_root.join("docs/alpha"), &["design", "plan", "done"]);

    app.reload_with_rediscovery()
        .expect("reload should succeed after a valid README rewrite");

    let snapshot = app.snapshot();
    let stages: Vec<String> = snapshot
        .definition
        .stages
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(stages, vec!["design", "plan", "done"]);
}

/// AC-2: a brand-new workflow directory created under the scan root is
/// picked up by the next reload, without restart.
#[test]
fn new_workflow_directory_appears_in_session() {
    let tmp = tempdir().unwrap();
    let scan_root = tmp.path();
    write_workflow_readme(&scan_root.join("docs/alpha"), &["design", "done"]);

    let mut app = build_session(scan_root);
    assert_eq!(app.as_session().unwrap().discovery().len(), 1);

    write_workflow_readme(&scan_root.join("docs/beta"), &["plan", "done"]);

    app.reload_with_rediscovery().expect("reload");

    let discovery = app.as_session().unwrap().discovery();
    assert_eq!(discovery.len(), 2, "beta must appear in session discovery");
    let roots = discovery_roots(discovery);
    let alpha_canon = canonical(&scan_root.join("docs/alpha"));
    let beta_canon = canonical(&scan_root.join("docs/beta"));
    assert!(
        roots.iter().any(|r| r == &alpha_canon),
        "alpha must remain present: {roots:?}"
    );
    assert!(
        roots.iter().any(|r| r == &beta_canon),
        "beta must appear: {roots:?}"
    );
}

/// AC-3a: when the active workflow is removed but a sibling remains,
/// reload falls back to the sibling without panicking.
#[test]
fn removing_active_workflow_yields_empty_state_without_panic() {
    let tmp = tempdir().unwrap();
    let scan_root = tmp.path();
    write_workflow_readme(&scan_root.join("docs/alpha"), &["design", "done"]);
    write_workflow_readme(&scan_root.join("docs/beta"), &["plan", "done"]);

    let mut app = build_session(scan_root);
    assert_eq!(app.as_session().unwrap().discovery().len(), 2);
    // Active is index 0; canonicalize since discovery returns canonical paths.
    let active_canon = canonical(&scan_root.join("docs/alpha"));
    assert_eq!(app.workflow_dir(), active_canon.as_path());

    // Remove alpha entirely.
    fs::remove_dir_all(scan_root.join("docs/alpha")).expect("remove alpha");

    app.reload_with_rediscovery().expect("reload");

    let session = app.as_session().expect("session");
    assert_eq!(
        session.discovery().len(),
        1,
        "alpha must be gone from discovery"
    );
    let beta_canon = canonical(&scan_root.join("docs/beta"));
    assert_eq!(
        session.active_dir(),
        beta_canon.as_path(),
        "active falls back to surviving workflow"
    );
    // Beta's stages should be loaded into the snapshot.
    let snapshot = app.snapshot();
    let stages: Vec<String> = snapshot
        .definition
        .stages
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(stages, vec!["plan", "done"]);
}

/// AC-3b: when the only workflow is removed, the session reaches a
/// non-panicking empty overview state with `last_refresh_error` set so the
/// UI can surface a clear message.
#[test]
fn removing_only_workflow_yields_empty_overview_with_error() {
    let tmp = tempdir().unwrap();
    let scan_root = tmp.path();
    write_workflow_readme(&scan_root.join("docs/only"), &["design", "done"]);

    let mut app = build_session(scan_root);

    fs::remove_dir_all(scan_root.join("docs/only")).expect("remove only");

    // The reload itself should not panic and should report Ok (the empty
    // synthetic state isn't a parse error, just an absence).
    app.reload_with_rediscovery().expect("reload");

    let snapshot = app.as_overview().unwrap().snapshot();
    let stages = &snapshot.definition.stages;
    assert!(
        stages.is_empty(),
        "synthetic empty state must have no stages, got {stages:?}"
    );
    assert!(
        app.last_refresh_error().is_some(),
        "a refresh error message must be surfaced after removal"
    );
}

/// AC-4: a malformed README during reload preserves the prior good
/// definition and records a warning the UI can display.
#[test]
fn malformed_readme_preserves_prior_definition() {
    let tmp = tempdir().unwrap();
    let scan_root = tmp.path();
    write_workflow_readme(&scan_root.join("docs/alpha"), &["design", "plan", "done"]);

    let mut app = build_session(scan_root);
    let prior_stages: Vec<String> = app
        .snapshot()
        .definition
        .stages
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(
        prior_stages,
        vec!["design".to_string(), "plan".to_string(), "done".to_string()]
    );

    // Overwrite the README with a frontmatter that parses cleanly at the
    // discovery layer (so the workflow stays in the discovery list) but
    // fails the workflow-definition parse because the `stages:` block is
    // gone — the parser returns `MissingRequiredField { field: "stages" }`.
    fs::write(
        scan_root.join("docs/alpha/README.md"),
        "---\ncommissioned-by: spacedock@0.10.1\n---\n\n# bad — missing stages\n",
    )
    .expect("write malformed README");

    // The reload returns Err but the prior good snapshot is preserved.
    let result = app.reload_with_rediscovery();
    assert!(
        result.is_err(),
        "malformed README must surface as Err, got {result:?}"
    );

    let stages_after: Vec<String> = app
        .snapshot()
        .definition
        .stages
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(
        stages_after, prior_stages,
        "prior good definition must be preserved on parse failure"
    );
    let err_msg = app.last_refresh_error().expect("an error must be recorded");
    assert!(
        err_msg.contains("stages") || err_msg.contains("README"),
        "error should mention the failing field or file, got: {err_msg}"
    );
}
