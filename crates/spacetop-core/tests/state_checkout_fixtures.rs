use std::fs;
use std::path::Path;
use std::process::Command;

use spacetop_core::domain::{StateCheckoutDisposition, WorkflowStorage};
use spacetop_core::index::WorkflowIndex;
use spacetop_core::parser::load_workflow_dir;
use spacetop_core::sources::WorkflowSources;
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

fn write_workflow(definition: &Path) {
    fs::create_dir_all(definition).expect("definition dir");
    fs::write(
        definition.join("README.md"),
        "---\ncommissioned-by: spacedock@0.26.0\nstate: .spacedock-state\nstages:\n  states:\n    - name: implement\n---\n# Demo\n",
    )
    .expect("README");
}

fn write_entities(state: &Path) {
    fs::create_dir_all(state.join("_archive")).expect("archive");
    fs::write(
        state.join("active.md"),
        "---\nid: active\ntitle: Active\nstatus: implement\n---\nactive body\n",
    )
    .expect("active");
    fs::write(
        state.join("_archive/archived.md"),
        "---\nid: archived\ntitle: Archived\nstatus: implement\n---\narchive body\n",
    )
    .expect("archive entity");
}

#[test]
fn real_git_attached_detached_wrong_and_missing_topologies_remain_truthful() {
    let temp = tempdir().expect("tempdir");
    let definition = temp.path().join("demo");
    let state = definition.join(".spacedock-state");
    write_workflow(&definition);
    fs::create_dir(&state).expect("state checkout");
    git(
        &state,
        &["init", "--initial-branch", "spacedock-state/demo"],
    );
    write_entities(&state);
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

    let attached = load_workflow_dir(&definition, temp.path()).expect("attached load");
    assert!(matches!(
        attached.definition.storage,
        WorkflowStorage::SplitRoot {
            disposition: StateCheckoutDisposition::Attached,
            ..
        }
    ));
    assert_eq!(attached.items.len(), 1);

    git(&state, &["checkout", "--detach"]);
    let detached = load_workflow_dir(&definition, temp.path()).expect("detached load");
    assert!(matches!(
        detached.definition.storage,
        WorkflowStorage::SplitRoot {
            disposition: StateCheckoutDisposition::Detached,
            ..
        }
    ));
    assert_eq!(detached.items[0].body.trim(), "active body");
    let detached_index = WorkflowIndex::load(&definition, temp.path()).expect("detached index");
    assert!(matches!(
        detached_index.storage(),
        WorkflowStorage::SplitRoot {
            disposition: StateCheckoutDisposition::Detached,
            ..
        }
    ));
    let archive = WorkflowSources::load_archive(&definition, &detached.definition);
    assert_eq!(archive.entities.len(), 1);

    git(&state, &["switch", "-c", "wrong-state"]);
    let wrong = load_workflow_dir(&definition, temp.path()).expect("wrong load");
    assert!(matches!(
        wrong.definition.storage,
        WorkflowStorage::SplitRoot {
            disposition: StateCheckoutDisposition::WrongBranch { ref actual_branch },
            ..
        } if actual_branch == "wrong-state"
    ));
    assert_eq!(
        wrong.items.len(),
        1,
        "wrong-branch snapshot remains readable"
    );

    fs::remove_dir_all(&state).expect("remove state checkout");
    let missing = load_workflow_dir(&definition, temp.path()).expect("missing load");
    assert!(matches!(
        missing.definition.storage,
        WorkflowStorage::SplitRoot {
            disposition: StateCheckoutDisposition::Missing,
            ..
        }
    ));
    assert!(missing.items.is_empty());
}

#[cfg(unix)]
#[test]
fn external_symlinked_git_checkout_is_unverified_and_remains_readable() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().expect("tempdir");
    let definition = temp.path().join("demo");
    let external_state = temp.path().join("external-state");
    write_workflow(&definition);
    write_entities(&external_state);
    git(
        &external_state,
        &["init", "--initial-branch", "spacedock-state/demo"],
    );
    symlink(&external_state, definition.join(".spacedock-state")).expect("state symlink");

    let snapshot = load_workflow_dir(&definition, temp.path()).expect("workflow load");

    assert!(matches!(
        snapshot.definition.storage,
        WorkflowStorage::SplitRoot {
            disposition: StateCheckoutDisposition::ProbeFailed { ref reason },
            ..
        } if reason.contains("resolves outside workflow definition directory")
    ));
    assert_eq!(snapshot.items.len(), 1, "non-holder content stays readable");
}
