use std::fs;
use std::path::Path;
use std::process::Command;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use spacetop::app::{App, StateTopologyDiagnostic, ViewScope};
use tempfile::tempdir;

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn reload_reprobes_topology_and_keeps_materialized_archive_readable() {
    let temp = tempdir().expect("tempdir");
    let definition = temp.path().join("demo");
    let state = definition.join(".spacedock-state");
    fs::create_dir_all(state.join("_archive")).expect("dirs");
    fs::write(
        definition.join("README.md"),
        "---\nstate: .spacedock-state\nstages:\n  states:\n    - name: implement\n---\n# Demo\n",
    )
    .expect("README");
    fs::write(
        state.join("active.md"),
        "---\nid: active\ntitle: Active\nstatus: implement\n---\nbody\n",
    )
    .expect("active");
    fs::write(
        state.join("_archive/archived.md"),
        "---\nid: archived\ntitle: Archived\nstatus: implement\n---\nbody\n",
    )
    .expect("archived");
    git(
        &state,
        &["init", "--initial-branch", "spacedock-state/demo"],
    );
    git(&state, &["add", "."]);
    git(
        &state,
        &[
            "-c",
            "user.name=Spacetop Test",
            "-c",
            "user.email=spacetop@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    );

    let mut app = App::load(definition).expect("load attached");
    assert!(app
        .as_overview()
        .expect("overview")
        .topology_diagnostic()
        .is_none());
    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(app.view_scope(), ViewScope::Archived);
    assert_eq!(app.visible_items().len(), 1);

    git(&state, &["checkout", "--detach"]);
    app.reload().expect("reload detached");
    assert_eq!(
        app.as_overview().expect("overview").topology_diagnostic(),
        Some(StateTopologyDiagnostic::Detached)
    );
    assert_eq!(app.view_scope(), ViewScope::Archived);
    assert_eq!(app.visible_items().len(), 1);

    let parked = definition_path(temp.path()).join("parked-state");
    fs::rename(&state, &parked).expect("hide state");
    app.reload().expect("reload missing");
    assert_eq!(
        app.as_overview().expect("overview").topology_diagnostic(),
        Some(StateTopologyDiagnostic::Missing)
    );
    assert!(app.visible_items().is_empty());

    fs::rename(&parked, &state).expect("restore state");
    app.reload().expect("reload restored");
    assert_eq!(
        app.as_overview().expect("overview").topology_diagnostic(),
        Some(StateTopologyDiagnostic::Detached)
    );
    assert_eq!(app.visible_items().len(), 1);
}

fn definition_path(root: &Path) -> std::path::PathBuf {
    root.join("demo")
}
