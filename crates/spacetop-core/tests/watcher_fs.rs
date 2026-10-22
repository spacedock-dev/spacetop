//! Real-backend integration test for `WorkflowWatcher`.
//!
//! Marked `#[ignore]` so CI runs the deterministic unit tests by default;
//! run locally with `cargo test -- --ignored` to exercise the live `notify`
//! backend on the current platform.

use std::fs;
use std::time::Duration;

use spacetop_core::watcher::{WatcherConfig, WorkflowWatcher};

#[test]
#[ignore = "exercises the real notify backend; run with --ignored locally"]
fn writes_to_markdown_trigger_refresh_signal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_watcher, rx) =
        WorkflowWatcher::start(dir.path(), WatcherConfig::default()).expect("start watcher");

    // Let the backend settle on its initial registration.
    std::thread::sleep(Duration::from_millis(50));

    let task_path = dir.path().join("task.md");
    fs::write(
        &task_path,
        "---\nid: 001\ntitle: Alpha\nstatus: plan\n---\nbody\n",
    )
    .expect("write task");

    // Debounce window is 250 ms; allow generous CI slack.
    let signal = rx.recv_timeout(Duration::from_millis(2000));
    assert!(
        signal.is_ok(),
        "expected a RefreshSignal after filesystem write"
    );
}

/// AC-1: external mutation of an entity file's frontmatter triggers a
/// refresh signal within the debounce window.
#[test]
#[ignore = "exercises the real notify backend; run with --ignored locally"]
fn external_frontmatter_edit_triggers_refresh() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Pre-create the entity file so the watcher sees a *modify*, not a
    // *create*. Mirrors a sibling tool editing an existing entity.
    let task_path = dir.path().join("task.md");
    fs::write(
        &task_path,
        "---\nid: 001\ntitle: Alpha\nstatus: design\n---\nbody\n",
    )
    .expect("seed task");

    let (_watcher, rx) =
        WorkflowWatcher::start(dir.path(), WatcherConfig::default()).expect("start watcher");

    // Settle initial registration; drain any synthetic startup signals.
    std::thread::sleep(Duration::from_millis(100));
    while rx.try_recv().is_ok() {}

    fs::write(
        &task_path,
        "---\nid: 001\ntitle: Alpha\nstatus: implement\n---\nbody\n",
    )
    .expect("rewrite frontmatter");

    let signal = rx.recv_timeout(Duration::from_millis(2000));
    assert!(
        signal.is_ok(),
        "expected a RefreshSignal after frontmatter rewrite"
    );
}

/// AC-2: a merge-style event sequence — renaming an entity into
/// `_archive/` plus adding a new entity file in the active dir — drives
/// at least one refresh signal. Two FS mutations may collapse into a
/// single signal under debounce; we assert arrival, not count.
#[test]
#[ignore = "exercises the real notify backend; run with --ignored locally"]
fn archive_rename_and_new_file_trigger_refresh() {
    let dir = tempfile::tempdir().expect("tempdir");
    let active = dir.path().join("task-old.md");
    fs::write(
        &active,
        "---\nid: 010\ntitle: Old\nstatus: implement\n---\nbody\n",
    )
    .expect("seed active task");

    let (_watcher, rx) =
        WorkflowWatcher::start(dir.path(), WatcherConfig::default()).expect("start watcher");

    std::thread::sleep(Duration::from_millis(100));
    while rx.try_recv().is_ok() {}

    // (a) Move the active entity into _archive/, mirroring a post-merge
    // archive move.
    let archive = dir.path().join("_archive");
    fs::create_dir_all(&archive).expect("mk archive dir");
    fs::rename(&active, archive.join("task-old.md")).expect("rename into archive");

    // (b) Materialize a brand-new active entity file.
    fs::write(
        dir.path().join("task-new.md"),
        "---\nid: 011\ntitle: New\nstatus: plan\n---\nbody\n",
    )
    .expect("write new task");

    let signal = rx.recv_timeout(Duration::from_millis(2000));
    assert!(
        signal.is_ok(),
        "expected at least one RefreshSignal after archive rename + new file"
    );
}

#[test]
#[ignore = "exercises real notify review-log append/rename/new-round/delete events"]
fn correction_history_changes_trigger_real_notify_refresh() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let room = dir.path().join("sample/review/implementation/round-1");
    fs::create_dir_all(&room).unwrap();
    let log = room.join("briefing.review.jsonl");
    fs::write(&log, "{}\n").unwrap();
    let (_watcher, rx) = WorkflowWatcher::start(dir.path(), WatcherConfig::default()).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let settle = || {
        std::thread::sleep(Duration::from_millis(400));
        while rx.try_recv().is_ok() {}
    };
    settle();
    fs::OpenOptions::new()
        .append(true)
        .open(&log)
        .unwrap()
        .write_all(b"{")
        .unwrap();
    assert!(
        rx.recv_timeout(Duration::from_secs(3)).is_ok(),
        "partial append"
    );
    settle();
    fs::OpenOptions::new()
        .append(true)
        .open(&log)
        .unwrap()
        .write_all(b"}\n")
        .unwrap();
    assert!(
        rx.recv_timeout(Duration::from_secs(3)).is_ok(),
        "tail completion"
    );
    settle();
    let temporary = room.join("replacement.tmp");
    fs::write(&temporary, "{}\n").unwrap();
    fs::rename(&temporary, &log).unwrap();
    assert!(
        rx.recv_timeout(Duration::from_secs(3)).is_ok(),
        "atomic rename"
    );
    settle();
    let next = dir.path().join("sample/review/implementation/round-2");
    fs::create_dir(&next).unwrap();
    fs::write(next.join("briefing.review.jsonl"), "{}\n").unwrap();
    assert!(rx.recv_timeout(Duration::from_secs(3)).is_ok(), "new round");
    settle();
    fs::remove_dir_all(next).unwrap();
    assert!(
        rx.recv_timeout(Duration::from_secs(3)).is_ok(),
        "round deletion"
    );
}

#[test]
#[ignore = "exercises the real notify polling backend on review JSONL"]
fn correction_history_poll_backend_observes_log_replacement() {
    use notify::{Config, PollWatcher, RecursiveMode, Watcher};
    let dir = tempfile::tempdir().unwrap();
    let room = dir.path().join("review/implementation/round-1");
    fs::create_dir_all(&room).unwrap();
    let log = room.join("briefing.review.jsonl");
    fs::write(&log, "{}\n").unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = PollWatcher::new(
        move |event| {
            let _ = tx.send(event);
        },
        Config::default()
            .with_poll_interval(Duration::from_millis(100))
            .with_compare_contents(true),
    )
    .unwrap();
    watcher.watch(dir.path(), RecursiveMode::Recursive).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    while rx.try_recv().is_ok() {}
    fs::write(&log, "{\"changed\":true}\n").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    loop {
        let event = rx
            .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .expect("poll event")
            .unwrap();
        if event.paths.contains(&log) {
            break;
        }
    }
}
