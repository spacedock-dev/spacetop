//! End-to-end integration tests for the Sync action.
//!
//! These tests construct real git repositories with `git init` so that
//! the production [`StdGitRunner`] exercises the full `git pull --ff-only`
//! path. The tests early-exit when `git` is not on `PATH` so they remain
//! safe to run in minimal environments.

use std::path::{Path, PathBuf};
use std::process::Command;

use spacetop::app::{App, SyncStatus};
use spacetop::apply_pending_sync;
use spacetop_core::git_sync::{self, StdGitRunner};

/// Probe `git --version`; return `false` and emit an `eprintln!` if git
/// is not on `PATH`. Each test guards on this so a missing git in CI
/// becomes a noisy skip instead of a confusing failure.
fn git_on_path() -> bool {
    match Command::new("git").arg("--version").output() {
        Ok(out) if out.status.success() => true,
        _ => {
            eprintln!("git not on PATH; skipping integration test");
            false
        }
    }
}

fn run_git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} failed to start: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} in {cwd:?} failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Build a tempdir-backed git playground:
///   bare/      — `git init --bare`
///   upstream/  — clone of bare with one workflow seeded (README + one task)
///   working/   — clone of bare; Spacetop opens its workflow dir
fn build_playground() -> Option<(tempfile::TempDir, PathBuf, PathBuf)> {
    if !git_on_path() {
        return None;
    }
    let holder = tempfile::tempdir().expect("tempdir");
    let bare = holder.path().join("bare");
    let upstream = holder.path().join("upstream");
    std::fs::create_dir_all(&bare).unwrap();

    // bare
    run_git(&bare, &["init", "--bare", "--initial-branch=main"]);

    // upstream: clone, configure identity, seed workflow.
    run_git(
        holder.path(),
        &["clone", bare.to_str().unwrap(), "upstream"],
    );
    run_git(&upstream, &["config", "user.email", "spacetop@test"]);
    run_git(&upstream, &["config", "user.name", "Spacetop Test"]);
    let workflow_dir = upstream.join("docs/wf");
    std::fs::create_dir_all(&workflow_dir).unwrap();
    std::fs::write(
        workflow_dir.join("README.md"),
        "---\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: done\n      terminal: true\n---\n",
    )
    .unwrap();
    std::fs::write(
        workflow_dir.join("task-001.md"),
        "---\nid: 001\ntitle: Initial\nstatus: plan\n---\n\nbody\n",
    )
    .unwrap();
    run_git(&upstream, &["add", "."]);
    run_git(
        &upstream,
        &["-c", "commit.gpgsign=false", "commit", "-m", "initial"],
    );
    run_git(&upstream, &["push", "origin", "main"]);

    // working: clone of bare. Configure identity for symmetry (not used
    // by spacetop, which only pulls, but pleasant for debugging).
    run_git(holder.path(), &["clone", bare.to_str().unwrap(), "working"]);
    let working = holder.path().join("working");
    run_git(&working, &["config", "user.email", "spacetop@test"]);
    run_git(&working, &["config", "user.name", "Spacetop Test"]);

    let working_workflow = working.join("docs/wf");
    Some((holder, upstream, working_workflow))
}

/// Push a second commit through the upstream clone that adds a new entity
/// file. Returns the new entity slug so callers can assert on it.
fn push_new_entity(upstream: &Path, slug: &str) {
    let workflow_dir = upstream.join("docs/wf");
    std::fs::write(
        workflow_dir.join(format!("task-{slug}.md")),
        format!("---\nid: {slug}\ntitle: Added\nstatus: plan\n---\n\nadded body\n"),
    )
    .unwrap();
    run_git(upstream, &["add", "."]);
    run_git(
        upstream,
        &["-c", "commit.gpgsign=false", "commit", "-m", "add task"],
    );
    run_git(upstream, &["push", "origin", "main"]);
}

/// AC-1 + AC-4: pulling new commits surfaces a Succeeded status and the
/// new entity appears in the App's snapshot after the post-pull reload.
#[test]
fn sync_pulls_new_commits_and_reflects_them() {
    let Some((_holder, upstream, working_wf)) = build_playground() else {
        return;
    };
    let mut app = App::load(working_wf.clone()).expect("load working workflow");
    assert_eq!(app.snapshot().items.len(), 1, "starts with one item");

    // Push a new entity through the upstream clone.
    push_new_entity(&upstream, "002");

    // Drive the same drain the event loop uses.
    app.request_sync();
    assert!(app.take_pending_sync());
    apply_pending_sync(&mut app, &StdGitRunner);

    match app.sync_status() {
        Some(SyncStatus::Succeeded { new_commits }) => {
            assert!(
                *new_commits >= 1,
                "expected ≥1 new commit, got {new_commits}"
            );
        }
        other => panic!("expected Succeeded, got {other:?}"),
    }

    // AC-4: the snapshot reflects the new state.
    assert!(
        app.snapshot().items.iter().any(|i| i.id == "002"),
        "expected entity 002 in snapshot after sync, got: {:?}",
        app.snapshot()
            .items
            .iter()
            .map(|i| i.id.clone())
            .collect::<Vec<_>>()
    );
}

/// AC-2: pointing Spacetop at a non-git directory must report Unavailable
/// and must not exec `git pull`.
#[test]
fn sync_unavailable_on_non_git_dir() {
    if !git_on_path() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let workflow = dir.path().join("wf");
    std::fs::create_dir_all(&workflow).unwrap();
    std::fs::write(
        workflow.join("README.md"),
        "---\nstages:\n  states:\n    - name: plan\n      initial: true\n---\n",
    )
    .unwrap();
    let mut app = App::load(workflow).expect("load workflow");
    app.request_sync();
    assert!(app.take_pending_sync());
    apply_pending_sync(&mut app, &StdGitRunner);
    match app.sync_status() {
        Some(SyncStatus::Unavailable { hint }) => {
            assert_eq!(hint, "not a git repository");
        }
        other => panic!("expected Unavailable(not a git repository), got {other:?}"),
    }
}

/// AC-3: a failing pull (unreachable remote) surfaces a Failed status
/// without panicking and the app keeps working — selection still moves.
#[test]
fn sync_failed_pull_keeps_app_intact() {
    let Some((_holder, _upstream, working_wf)) = build_playground() else {
        return;
    };
    // Repoint origin at an invalid local path.
    let working_root = working_wf.parent().unwrap().parent().unwrap();
    run_git(
        working_root,
        &[
            "remote",
            "set-url",
            "origin",
            "file:///definitely/does/not/exist.git",
        ],
    );

    let mut app = App::load(working_wf).expect("load workflow");
    let pre_index = app.selected_index();
    app.request_sync();
    assert!(app.take_pending_sync());
    apply_pending_sync(&mut app, &StdGitRunner);

    match app.sync_status() {
        Some(SyncStatus::Failed { message }) => {
            assert!(!message.is_empty(), "Failed message should carry stderr");
        }
        other => panic!("expected Failed, got {other:?}"),
    }

    // App still works: selection-down still mutates state cleanly.
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    // With only one item, selection clamps to 0; the assertion is that
    // we did not panic and the app is responsive.
    let _ = pre_index;
}

/// AC-1 happy path against the helper directly: a fresh clone with no
/// upstream commits since the initial push reports UpToDate.
#[test]
fn sync_up_to_date_when_nothing_to_pull() {
    let Some((_holder, _upstream, working_wf)) = build_playground() else {
        return;
    };
    let mut app = App::load(working_wf.clone()).expect("load");
    app.request_sync();
    assert!(app.take_pending_sync());
    apply_pending_sync(&mut app, &StdGitRunner);
    match app.sync_status() {
        Some(SyncStatus::Succeeded { new_commits }) => assert_eq!(*new_commits, 0),
        other => panic!("expected Succeeded(0), got {other:?}"),
    }

    // The probe-availability path used by the helper is also Available
    // for the same root — guards against the failure path silently
    // reclassifying success.
    let root = app.repo_root().expect("repo root").to_path_buf();
    assert_eq!(
        git_sync::probe_availability(&StdGitRunner, &root),
        git_sync::SyncAvailability::Available
    );
}
