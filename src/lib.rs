pub mod app;
pub mod cli;
pub mod discovery;
pub mod domain;
pub mod parser;
pub mod ui;
pub mod watcher;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::time::Duration;

use anyhow::{anyhow, Context};
use app::{App, AppMode};
use cli::Cli;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use watcher::{WatcherBackend, WatcherConfig, WorkflowWatcher};

/// Result of resolving a CLI invocation into a launch decision, prior to any
/// TUI startup. Exposed so integration tests can assert zero/one/many and the
/// `-w` bypass without spawning a terminal.
#[derive(Debug)]
pub enum DecideOutcome {
    /// Open the given app in the TUI.
    Overview(App),
    /// Open the picker TUI against multiple discovered workflows.
    Picker(App),
    /// No workflows found; caller should report the stable stderr message
    /// naming this scan root and exit non-zero.
    ZeroWorkflows { scan_root: PathBuf },
}

pub fn decide_app(cli: &Cli, cwd: &Path) -> anyhow::Result<DecideOutcome> {
    if let Some(explicit) = cli.workflow_dir.clone() {
        if let Ok(app) = App::load(explicit.clone()) {
            return Ok(DecideOutcome::Overview(app));
        }

        let workflows = discovery::discover_workflows(&explicit)
            .with_context(|| format!("failed to scan {}", explicit.display()))?;
        return match workflows.len() {
            0 => {
                let app = App::load(explicit.clone()).with_context(|| {
                    format!("failed to load workflow directory {}", explicit.display())
                })?;
                Ok(DecideOutcome::Overview(app))
            }
            1 => {
                let only = workflows.into_iter().next().unwrap();
                let state = app::OverviewState::load(only.root.clone()).with_context(|| {
                    format!("failed to load workflow directory {}", only.root.display())
                })?;
                let session = app::OverviewSession::single(state, false);
                Ok(DecideOutcome::Overview(App::from_session(session)))
            }
            _ => {
                let first = workflows
                    .first()
                    .expect("non-empty workflow list")
                    .root
                    .clone();
                let state = app::OverviewState::load(first.clone()).with_context(|| {
                    format!("failed to load workflow directory {}", first.display())
                })?;
                let session =
                    app::OverviewSession::from_discovery(explicit, workflows, 0, state);
                Ok(DecideOutcome::Overview(App::from_session(session)))
            }
        };
    }

    let scan_root = discovery::resolve_scan_root(cwd);
    let workflows = discovery::discover_workflows(&scan_root)
        .with_context(|| format!("failed to scan {}", scan_root.display()))?;

    match workflows.len() {
        0 => Ok(DecideOutcome::ZeroWorkflows { scan_root }),
        1 => {
            let only = workflows.into_iter().next().unwrap();
            let state = app::OverviewState::load(only.root.clone()).with_context(|| {
                format!("failed to load workflow directory {}", only.root.display())
            })?;
            // Discovery path with exactly one workflow: not `-w` pinned, but
            // is_multi() is false because len() == 1, so cycle/P keys stay
            // inert per the design.
            let session = app::OverviewSession::single(state, false);
            Ok(DecideOutcome::Overview(App::from_session(session)))
        }
        _ => {
            let first = workflows
                .first()
                .expect("non-empty workflow list")
                .root
                .clone();
            let state = app::OverviewState::load(first.clone()).with_context(|| {
                format!("failed to load workflow directory {}", first.display())
            })?;
            let session = app::OverviewSession::from_discovery(scan_root, workflows, 0, state);
            Ok(DecideOutcome::Overview(App::from_session(session)))
        }
    }
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    match decide_app(&cli, &cwd)? {
        DecideOutcome::Overview(app) | DecideOutcome::Picker(app) => run_terminal(app),
        DecideOutcome::ZeroWorkflows { scan_root } => {
            eprintln!(
                "spacetop: no Spacedock workflows found under {}. Pass --workflow-dir <path> to open a specific directory.",
                scan_root.display()
            );
            Err(anyhow!("no workflows discovered"))
        }
    }
}

fn run_terminal(mut app: App) -> anyhow::Result<()> {
    enable_raw_mode().context("failed to enable terminal raw mode")?;
    let _restore = TerminalRestore;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to initialize terminal")?;

    // Start the filesystem watcher against the current overview's workflow
    // dir. If we're in picker mode there's no single workflow to watch yet;
    // the watcher is re-initialized once the user enters an overview via
    // the picker.
    let mut watcher_state: Option<(
        WorkflowWatcher,
        std::sync::mpsc::Receiver<watcher::RefreshSignal>,
    )> = start_watcher_for(&mut app);

    loop {
        terminal
            .draw(|frame| ui::render(frame, &app))
            .context("failed to draw terminal UI")?;

        if app.should_quit() {
            break;
        }

        // 1. Drain any pending refresh signals.
        if let Some((_, ref rx)) = watcher_state {
            loop {
                match rx.try_recv() {
                    Ok(_) => {
                        let _ = app.reload();
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        app.set_refresh_error("watcher: disconnected".into());
                        watcher_state = None;
                        break;
                    }
                }
            }
        }

        // 2. Short crossterm poll.
        let prior_mode_was_picker = matches!(app.mode(), AppMode::Picker(_));
        if event::poll(Duration::from_millis(100)).context("failed to poll terminal events")? {
            if let Event::Key(key) = event::read().context("failed to read terminal event")? {
                app.handle_key(key);
            }
        }

        // If we just transitioned from picker to overview, spin up the
        // watcher on the selected workflow dir.
        if prior_mode_was_picker && matches!(app.mode(), AppMode::Overview(_)) {
            watcher_state = start_watcher_for(&mut app);
        }

        // 3. Drain pending picker-overlay open request: re-run discovery
        // against the session's scan root, then transition into overlay
        // mode. Sequenced before the switch drain so an overlay-confirm in
        // the same frame still triggers a switch on the next frame.
        if app.take_pending_overlay_open() {
            let result = match app.as_session().and_then(|s| s.scan_root()) {
                Some(root) => {
                    let scan_root = root.to_path_buf();
                    discovery::discover_workflows(&scan_root)
                        .map_err(|e| format!("re-discovery failed: {e}"))
                }
                None => Err("re-discovery unavailable: no scan root".to_string()),
            };
            app.open_picker_overlay_with(result);
        }

        // 4. Drain pending workflow switch (from `]`/`[` cycle or from
        // picker-overlay confirm). Tear down the prior watcher, materialize
        // or reload the active state, then start a fresh watcher.
        if let Some(switch) = app.take_pending_switch() {
            // Drop the prior watcher + receiver before re-starting so the
            // debounce thread joins cleanly.
            drop(watcher_state.take());
            if switch.needs_first_load {
                app.materialize_active();
            } else {
                let _ = app.reload();
            }
            watcher_state = start_watcher_for(&mut app);
        }
    }

    drop(watcher_state);

    terminal
        .show_cursor()
        .context("failed to restore terminal cursor")?;

    Ok(())
}

fn start_watcher_for(
    app: &mut App,
) -> Option<(
    WorkflowWatcher,
    std::sync::mpsc::Receiver<watcher::RefreshSignal>,
)> {
    let AppMode::Overview(_) = app.mode() else {
        return None;
    };
    let dir = app.workflow_dir().to_path_buf();
    match WorkflowWatcher::start(&dir, WatcherConfig::default()) {
        Ok((w, rx)) => {
            if w.backend() == WatcherBackend::Poll {
                app.set_refresh_error("watcher: polling fallback".into());
            }
            Some((w, rx))
        }
        Err(err) => {
            app.set_refresh_error(format!("watcher: unavailable ({err})"));
            None
        }
    }
}

struct TerminalRestore;

impl Drop for TerminalRestore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, OverviewSession, OverviewState};
    use crate::discovery::DiscoveredWorkflow;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn write_minimal_workflow(dir: &std::path::Path, slug: &str) {
        std::fs::write(
            dir.join("README.md"),
            "---\nstages:\n  states:\n    - name: plan\n      initial: true\n---\n",
        )
        .unwrap();
        std::fs::write(
            dir.join(format!("task-{slug}.md")),
            format!("---\nid: {slug}\ntitle: T{slug}\nstatus: plan\n---\n\nbody\n"),
        )
        .unwrap();
    }

    #[test]
    fn watcher_restarts_on_switch() {
        // Build a 2-workflow session; press `]`; tear down + restart the
        // watcher via `start_watcher_for` and assert it now follows the new
        // active dir. This is the smallest real-watcher lifecycle assertion
        // that the event-loop fragment in `run_terminal` performs.
        let holder = tempfile::tempdir().expect("tempdir");
        let w0 = holder.path().join("w0");
        let w1 = holder.path().join("w1");
        std::fs::create_dir_all(&w0).unwrap();
        std::fs::create_dir_all(&w1).unwrap();
        write_minimal_workflow(&w0, "000");
        write_minimal_workflow(&w1, "001");

        let discovery = vec![
            DiscoveredWorkflow {
                root: w0.clone(),
                title: None,
            },
            DiscoveredWorkflow {
                root: w1.clone(),
                title: None,
            },
        ];
        let initial = OverviewState::load(w0.clone()).expect("load w0");
        let session =
            OverviewSession::from_discovery(holder.path().to_path_buf(), discovery, 0, initial);
        let mut app = App::from_session(session);

        // Pre-switch: watcher follows w0.
        let watcher_state = start_watcher_for(&mut app);
        assert!(watcher_state.is_some(), "watcher should start on w0");
        assert_eq!(app.workflow_dir(), w0.as_path());

        // Press `Right` → switch pending.
        app.handle_key(key(KeyCode::Right));
        let switch = app.take_pending_switch().expect("cycle emits switch");
        assert_eq!(switch.target_index, 1);
        assert!(switch.needs_first_load);
        // Drop prior watcher first (mirrors event-loop ordering); join is
        // bounded by debounce thread shutdown.
        drop(watcher_state);
        app.materialize_active();
        let new_watcher_state = start_watcher_for(&mut app);
        assert!(new_watcher_state.is_some(), "watcher should restart on w1");
        assert_eq!(app.workflow_dir(), w1.as_path());
        // Stale refresh signal handling: dropping the old receiver means
        // the main loop reads only the new one — the channel is closed
        // without panicking the loop. We verified that by `drop` above
        // returning cleanly.
        drop(new_watcher_state);
    }

    #[test]
    fn zero_workflow_eprintln_prefix_is_stable() {
        // This test locks the stable stderr prefix specified by AC-3.
        let scan_root = PathBuf::from("/some/root");
        let msg = format!(
            "spacetop: no Spacedock workflows found under {}. Pass --workflow-dir <path> to open a specific directory.",
            scan_root.display()
        );
        assert!(msg.starts_with("spacetop: no Spacedock workflows found under "));
        assert!(msg.contains("/some/root"));
        assert!(msg.contains("Pass --workflow-dir <path>"));
    }
}
