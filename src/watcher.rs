//! Filesystem watcher for the workflow directory.
//!
//! Owns a `notify::RecommendedWatcher` (with `PollWatcher` fallback on backend
//! init failure) plus a background debounce thread. Raw notify events are
//! filtered to relevant workflow paths, coalesced over a configurable
//! trailing-edge window (default 250 ms), and each coalesced burst is
//! delivered as a single [`RefreshSignal`] on an `mpsc::Receiver` the caller
//! drains from the main event loop.
//!
//! The watcher is UI- and `App`-agnostic — it only says "something relevant
//! changed"; the caller decides what to reload.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};

/// Trailing-edge debounce window. Burst of events within this window
/// collapses to one [`RefreshSignal`]. `Duration::ZERO` disables timing
/// (every relevant raw event produces one signal) — used by tests.
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(250);

/// Poll interval used when falling back to `notify::PollWatcher`.
const POLL_FALLBACK_INTERVAL: Duration = Duration::from_secs(1);

/// Signal sent to the main loop when one or more relevant workflow files
/// have changed and the snapshot should be reloaded.
#[derive(Debug, Clone, Copy)]
pub struct RefreshSignal;

#[derive(Debug, Clone)]
pub struct WatcherConfig {
    pub debounce: Duration,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            debounce: DEFAULT_DEBOUNCE,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WatcherError {
    #[error("failed to initialize filesystem watcher backend: {0}")]
    BackendInit(String),
    #[error("failed to start watching {path}: {source}")]
    Watch {
        path: PathBuf,
        #[source]
        source: notify::Error,
    },
}

/// Indicates which backend a running [`WorkflowWatcher`] ended up using.
/// Exposed for the main loop's startup status hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherBackend {
    Recommended,
    Poll,
}

/// Owns the live `notify` watcher plus the debounce thread. Dropping the
/// watcher signals shutdown and joins the thread cleanly.
pub struct WorkflowWatcher {
    _watcher: Box<dyn Watcher + Send>,
    thread: Option<JoinHandle<()>>,
    shutdown: Sender<()>,
    backend: WatcherBackend,
}

impl WorkflowWatcher {
    /// Start watching `root` recursively. Returns the watcher plus the
    /// receiver the main loop drains.
    pub fn start(
        root: &Path,
        config: WatcherConfig,
    ) -> Result<(Self, Receiver<RefreshSignal>), WatcherError> {
        let (raw_tx, raw_rx) = mpsc::channel::<Event>();
        let (signal_tx, signal_rx) = mpsc::channel::<RefreshSignal>();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let raw_tx_for_backend = raw_tx.clone();
        let event_handler = move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let _ = raw_tx_for_backend.send(event);
            }
        };

        let (watcher_box, backend): (Box<dyn Watcher + Send>, WatcherBackend) =
            match RecommendedWatcher::new(event_handler.clone(), Config::default()) {
                Ok(mut w) => {
                    w.watch(root, RecursiveMode::Recursive)
                        .map_err(|e| WatcherError::Watch {
                            path: root.to_path_buf(),
                            source: e,
                        })?;
                    (Box::new(w), WatcherBackend::Recommended)
                }
                Err(recommended_err) => {
                    let poll_config = Config::default().with_poll_interval(POLL_FALLBACK_INTERVAL);
                    match PollWatcher::new(event_handler, poll_config) {
                        Ok(mut w) => {
                            w.watch(root, RecursiveMode::Recursive).map_err(|e| {
                                WatcherError::Watch {
                                    path: root.to_path_buf(),
                                    source: e,
                                }
                            })?;
                            (Box::new(w), WatcherBackend::Poll)
                        }
                        Err(_) => {
                            return Err(WatcherError::BackendInit(recommended_err.to_string()));
                        }
                    }
                }
            };

        let debounce = config.debounce;
        let thread = thread::spawn(move || {
            debounce_loop(raw_rx, signal_tx, shutdown_rx, debounce);
        });

        Ok((
            Self {
                _watcher: watcher_box,
                thread: Some(thread),
                shutdown: shutdown_tx,
                backend,
            },
            signal_rx,
        ))
    }

    pub fn backend(&self) -> WatcherBackend {
        self.backend
    }
}

impl Drop for WorkflowWatcher {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Return true if a path changed on disk should trigger a workflow reload.
///
/// Rules:
/// - `*.md` files anywhere under the watched root.
/// - Directories whose basename matches `^[A-Za-z0-9._-]+$` (for
///   archived-folder appearance or folder-form `{slug}/index.md`).
/// - Explicitly reject editor backup / swap / sentinel files even if they
///   would otherwise match.
pub(crate) fn is_relevant(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    // Reject common editor cruft first.
    if name == ".DS_Store" || name == "4913" {
        return false;
    }
    if name.ends_with('~') {
        return false;
    }
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lc = ext.to_ascii_lowercase();
        if matches!(ext_lc.as_str(), "swp" | "swx" | "swo" | "tmp" | "bak") {
            return false;
        }
        if ext_lc == "md" {
            return true;
        }
    }

    // Directory-like path (or file without extension): accept if the name
    // looks like a workflow slug / archive dir.
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

fn event_is_relevant(event: &Event) -> bool {
    // Ignore pure access events (macOS FSEvents sometimes emits these).
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    event.paths.iter().any(|p| is_relevant(p))
}

fn debounce_loop(
    raw_rx: Receiver<Event>,
    signal_tx: Sender<RefreshSignal>,
    shutdown_rx: Receiver<()>,
    debounce: Duration,
) {
    loop {
        // Block until an event arrives (or shutdown).
        let first = match raw_rx.recv() {
            Ok(ev) => ev,
            Err(_) => return,
        };
        if shutdown_rx.try_recv().is_ok() {
            return;
        }
        if !event_is_relevant(&first) {
            continue;
        }

        if debounce.is_zero() {
            if signal_tx.send(RefreshSignal).is_err() {
                return;
            }
            continue;
        }

        // Collect a burst. Reset deadline on each additional relevant event.
        let mut deadline = Instant::now() + debounce;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match raw_rx.recv_timeout(remaining) {
                Ok(ev) => {
                    if event_is_relevant(&ev) {
                        deadline = Instant::now() + debounce;
                    }
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    let _ = signal_tx.send(RefreshSignal);
                    return;
                }
            }
            if shutdown_rx.try_recv().is_ok() {
                return;
            }
        }

        if signal_tx.send(RefreshSignal).is_err() {
            return;
        }
    }
}

/// Drain any outstanding raw events on `raw_rx` up to `max`. Returns the
/// count drained. Used by tests to flush the channel.
#[cfg(test)]
fn drain<T>(rx: &Receiver<T>) -> usize {
    let mut n = 0;
    while rx.try_recv().is_ok() {
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::mpsc::TryRecvError;

    #[test]
    fn is_relevant_accepts_markdown_files() {
        assert!(is_relevant(Path::new("docs/spacetop-dev/task.md")));
        assert!(is_relevant(Path::new("task.MD")));
    }

    #[test]
    fn is_relevant_rejects_editor_backups_and_cruft() {
        assert!(!is_relevant(Path::new(".DS_Store")));
        assert!(!is_relevant(Path::new("docs/.DS_Store")));
        assert!(!is_relevant(Path::new("task.md.swp")));
        assert!(!is_relevant(Path::new("task.md.swx")));
        assert!(!is_relevant(Path::new("task.md~")));
        assert!(!is_relevant(Path::new("4913")));
    }

    #[test]
    fn is_relevant_accepts_workflow_like_directories() {
        // Directory-like basename with slug-safe chars.
        assert!(is_relevant(Path::new("docs/spacetop-dev/_archive")));
        assert!(is_relevant(Path::new(
            "docs/spacetop-dev/add-archived-tasks-view"
        )));
        // Basename with an invalid character is rejected.
        assert!(!is_relevant(Path::new("docs/workflow/bad name")));
    }

    #[test]
    fn debounce_coalesces_burst_into_single_signal() {
        // Use a short but real debounce window; inject synthetic events on
        // the raw channel to avoid dependence on a live `notify` backend.
        let (raw_tx, raw_rx) = mpsc::channel::<Event>();
        let (sig_tx, sig_rx) = mpsc::channel::<RefreshSignal>();
        let (shut_tx, shut_rx) = mpsc::channel::<()>();
        let debounce = Duration::from_millis(60);

        let handle = thread::spawn(move || {
            debounce_loop(raw_rx, sig_tx, shut_rx, debounce);
        });

        // Send a burst of 5 relevant events spaced 10 ms apart.
        for _ in 0..5 {
            raw_tx
                .send(
                    Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                        .add_path(PathBuf::from("/tmp/workflow/task.md")),
                )
                .unwrap();
            thread::sleep(Duration::from_millis(10));
        }

        // Expect exactly one signal within a generous window.
        let first = sig_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("expected one refresh signal");
        let _ = first;
        // Ensure no second signal arrives within the next debounce window.
        assert!(matches!(
            sig_rx.recv_timeout(Duration::from_millis(200)),
            Err(RecvTimeoutError::Timeout)
        ));

        // Shut the thread down cleanly.
        let _ = shut_tx.send(());
        drop(raw_tx);
        let _ = handle.join();
    }

    #[test]
    fn debounce_zero_emits_per_event() {
        let (raw_tx, raw_rx) = mpsc::channel::<Event>();
        let (sig_tx, sig_rx) = mpsc::channel::<RefreshSignal>();
        let (shut_tx, shut_rx) = mpsc::channel::<()>();

        let handle = thread::spawn(move || {
            debounce_loop(raw_rx, sig_tx, shut_rx, Duration::ZERO);
        });

        for _ in 0..3 {
            raw_tx
                .send(
                    Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                        .add_path(PathBuf::from("/tmp/workflow/task.md")),
                )
                .unwrap();
        }

        // Allow the loop to process.
        thread::sleep(Duration::from_millis(50));
        let received = drain(&sig_rx);
        assert_eq!(received, 3);

        let _ = shut_tx.send(());
        drop(raw_tx);
        let _ = handle.join();
    }

    #[test]
    fn debounce_ignores_irrelevant_events() {
        let (raw_tx, raw_rx) = mpsc::channel::<Event>();
        let (sig_tx, sig_rx) = mpsc::channel::<RefreshSignal>();
        let (shut_tx, shut_rx) = mpsc::channel::<()>();

        let handle = thread::spawn(move || {
            debounce_loop(raw_rx, sig_tx, shut_rx, Duration::ZERO);
        });

        raw_tx
            .send(
                Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                    .add_path(PathBuf::from("/tmp/workflow/.DS_Store")),
            )
            .unwrap();

        thread::sleep(Duration::from_millis(50));
        assert!(matches!(sig_rx.try_recv(), Err(TryRecvError::Empty)));

        let _ = shut_tx.send(());
        drop(raw_tx);
        let _ = handle.join();
    }

    #[test]
    fn start_real_backend_against_tempdir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (watcher, _rx) =
            WorkflowWatcher::start(dir.path(), WatcherConfig::default()).expect("watcher");
        assert!(matches!(
            watcher.backend(),
            WatcherBackend::Recommended | WatcherBackend::Poll
        ));
    }
}
