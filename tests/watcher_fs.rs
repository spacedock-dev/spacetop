//! Real-backend integration test for `WorkflowWatcher`.
//!
//! Marked `#[ignore]` so CI runs the deterministic unit tests by default;
//! run locally with `cargo test -- --ignored` to exercise the live `notify`
//! backend on the current platform.

use std::fs;
use std::time::Duration;

use spacetop::watcher::{WatcherConfig, WorkflowWatcher};

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
