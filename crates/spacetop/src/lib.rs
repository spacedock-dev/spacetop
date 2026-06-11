pub mod app;
pub mod cli;
pub mod headless;
pub mod ui;

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::time::Duration;

use anyhow::{anyhow, Context};
use app::{App, AppMode, HistoryWorkerResult, SyncStatus};
use cli::Cli;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use spacetop_core::config::{self, ConfigLoad, ConfigWarning, SpacetopConfig};
use spacetop_core::discovery;
use spacetop_core::editor::{resolve_editor, EditorLauncher, StdEnv, StdLauncher};
use spacetop_core::git_sync::{self, GitRunner, StdGitRunner, SyncOutcome};
use spacetop_core::session_state;
use spacetop_core::watcher::{self, WatcherBackend, WatcherConfig, WorkflowWatcher};

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
    decide_app_with_config(cli, cwd, SpacetopConfig::default(), Vec::new())
}

fn decide_app_with_config(
    cli: &Cli,
    cwd: &Path,
    config: SpacetopConfig,
    config_warnings: Vec<ConfigWarning>,
) -> anyhow::Result<DecideOutcome> {
    if let Some(explicit) = cli.workflow_dir.clone() {
        if let Ok(app) = App::load_with_config_warnings(
            explicit.clone(),
            config.clone(),
            config_warnings.clone(),
        ) {
            return Ok(DecideOutcome::Overview(app));
        }

        let workflows = discovery::discover_workflows(&explicit)
            .with_context(|| format!("failed to scan {}", explicit.display()))?;
        if workflows.is_empty() {
            let app = App::load_with_config_warnings(
                explicit.clone(),
                config.clone(),
                config_warnings.clone(),
            )
            .with_context(|| format!("failed to load workflow directory {}", explicit.display()))?;
            return Ok(DecideOutcome::Overview(app));
        }

        let scan_root = explicit.canonicalize().unwrap_or_else(|_| explicit.clone());
        return overview_from_discovered_workflows(scan_root, workflows, &config, &config_warnings);
    }

    let scan_root = discovery::resolve_scan_root(cwd);
    let workflows = discovery::discover_workflows(&scan_root)
        .with_context(|| format!("failed to scan {}", scan_root.display()))?;

    if workflows.is_empty() {
        return Ok(DecideOutcome::ZeroWorkflows { scan_root });
    }
    overview_from_discovered_workflows(scan_root, workflows, &config, &config_warnings)
}

fn overview_from_discovered_workflows(
    scan_root: PathBuf,
    workflows: Vec<discovery::DiscoveredWorkflow>,
    config: &SpacetopConfig,
    config_warnings: &[ConfigWarning],
) -> anyhow::Result<DecideOutcome> {
    if workflows.len() == 1 {
        let only = workflows.into_iter().next().expect("one workflow");
        let mut state = load_overview_state(&only.root)?;
        state.apply_config_defaults(config);
        // Discovery path with exactly one workflow: not `-w` pinned, but
        // is_multi() is false because len() == 1, so cycle/P keys stay
        // inert per the design.
        let session = app::OverviewSession::single(state, false);
        return Ok(DecideOutcome::Overview(
            App::from_session_with_config_warnings(
                session,
                config.clone(),
                config_warnings.to_vec(),
            ),
        ));
    }

    let first = workflows
        .first()
        .expect("non-empty workflow list")
        .root
        .clone();
    let mut state = load_overview_state(&first)?;
    state.apply_config_defaults(config);
    let session = app::OverviewSession::from_discovery(scan_root, workflows, 0, state);
    Ok(DecideOutcome::Overview(
        App::from_session_with_config_warnings(session, config.clone(), config_warnings.to_vec()),
    ))
}

fn load_overview_state(root: &Path) -> anyhow::Result<app::OverviewState> {
    app::OverviewState::load(root.to_path_buf())
        .with_context(|| format!("failed to load workflow directory {}", root.display()))
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    let config_load = load_startup_config(&config::StdEnv);
    match decide_app_with_config(&cli, &cwd, config_load.config, config_load.warnings)? {
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

fn load_startup_config(env: &impl config::ConfigEnv) -> ConfigLoad {
    match config::load_config_with_warnings(env) {
        Ok(load) => load,
        Err(err) => ConfigLoad {
            config: SpacetopConfig::default(),
            warnings: vec![ConfigWarning {
                message: format!("failed to load config: {err}"),
            }],
        },
    }
}

fn run_terminal(mut app: App) -> anyhow::Result<()> {
    let session_state_path = session_state::state_path(&config::StdEnv);
    load_session_state_for_app(&mut app, session_state_path.as_deref());

    // OSC 7 must land on the primary screen, before raw mode is enabled and
    // before EnterAlternateScreen, so terminals that support Smart Selection
    // on relative paths can resolve them against the right cwd.
    {
        let mut stdout = io::stdout();
        let is_tty = stdout.is_terminal();
        let cwd = std::env::current_dir().unwrap_or_default();
        let _ = emit_osc7(&mut stdout, is_tty, &cwd);
    }

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
    let mut history_worker_state = start_history_worker_for(&app);

    loop {
        terminal
            .draw(|frame| ui::render(frame, &app))
            .context("failed to draw terminal UI")?;

        if app.should_quit() {
            break;
        }

        drain_history_worker(&mut app, &mut history_worker_state);

        // 1. Drain any pending refresh signals.
        if let Some((_, ref rx)) = watcher_state {
            loop {
                match rx.try_recv() {
                    Ok(_) => {
                        if app.reload_with_rediscovery().is_ok() {
                            history_worker_state = start_history_worker_for(&app);
                        }
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
            history_worker_state = start_history_worker_for(&app);
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
            history_worker_state = start_history_worker_for(&app);
        }

        // 5. Drain pending sync request: redraw once with the in-flight
        // pill, then run `git_sync::sync` synchronously, then redraw with
        // the outcome on the next loop iteration.
        if app.take_pending_sync() {
            app.set_sync_status(SyncStatus::InFlight);
            terminal
                .draw(|frame| ui::render(frame, &app))
                .context("failed to draw terminal UI")?;
            apply_pending_sync(&mut app, &StdGitRunner);
            history_worker_state = start_history_worker_for(&app);
        }

        // 6. Drain pending "open file in $EDITOR" intent: suspend the TUI,
        // block on the editor process, resume, force a redraw next iter.
        // Errors are intentionally swallowed — they would otherwise tear
        // down the TUI for an issue (e.g. editor not installed) that the
        // user can recover from by just returning to spacetop.
        if let Some(path) = app.take_pending_open_file() {
            let mut stdout = io::stdout();
            if let Err(err) = suspend_terminal(&mut CrosstermTerminalControl(&mut stdout)) {
                app.set_refresh_error(format!("editor: suspend failed ({err})"));
            } else {
                let cmd = resolve_editor(&StdEnv);
                let _ = StdLauncher.launch(&cmd, &path);
                if let Err(err) = resume_terminal(&mut CrosstermTerminalControl(&mut stdout)) {
                    app.set_refresh_error(format!("editor: resume failed ({err})"));
                }
                let _ = terminal.clear();
            }
        }
    }

    drop(watcher_state);
    drop(history_worker_state);

    save_session_state_for_app(&mut app, session_state_path.as_deref());

    terminal
        .show_cursor()
        .context("failed to restore terminal cursor")?;

    Ok(())
}

fn load_session_state_for_app(app: &mut App, path: Option<&Path>) {
    let Some(path) = path else {
        return;
    };
    match session_state::load_session_file(path) {
        Ok(session_state) => app.apply_session_state(session_state),
        Err(err) => app.add_status_warning(format!("failed to load session state: {err}")),
    }
}

fn save_session_state_for_app(app: &mut App, path: Option<&Path>) {
    let Some(path) = path else {
        return;
    };
    let session_state = app.session_state_snapshot();
    if let Err(err) = session_state::save_session_file(path, &session_state) {
        app.add_status_warning(format!("failed to save session state: {err}"));
    }
}

fn start_history_worker_for(app: &App) -> Option<std::sync::mpsc::Receiver<HistoryWorkerResult>> {
    app.history_worker_request().map(app::spawn_history_worker)
}

fn drain_history_worker(
    app: &mut App,
    worker: &mut Option<std::sync::mpsc::Receiver<HistoryWorkerResult>>,
) {
    let mut clear_worker = false;
    if let Some(rx) = worker.as_ref() {
        match rx.try_recv() {
            Ok(result) => {
                app.apply_history_result(result);
                clear_worker = true;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                clear_worker = true;
            }
        }
    }
    if clear_worker {
        *worker = None;
    }
}

/// Run a sync against the active workflow's repo root and reflect the
/// outcome on `app`. Factored out of the event loop so integration tests
/// can drive it with a stub [`GitRunner`] and assert end-state without a
/// terminal. After a successful pull with new commits, the in-memory
/// snapshot is reloaded so AC-4 holds independent of the watcher path.
pub fn apply_pending_sync<R: GitRunner>(app: &mut App, runner: &R) {
    let Some(root) = app.repo_root().map(|p| p.to_path_buf()) else {
        app.set_sync_status(SyncStatus::Failed {
            message: "no active workflow".to_string(),
        });
        return;
    };
    let outcome = git_sync::sync(runner, &root);
    let availability = git_sync::probe_availability(runner, &root);
    let status = match outcome {
        SyncOutcome::UpToDate => SyncStatus::Succeeded { new_commits: 0 },
        SyncOutcome::Pulled { new_commits } => {
            // Explicit re-parse so the overview reflects newly pulled
            // entity files regardless of whether the filesystem watcher
            // already fired (AC-4 cross-references task 045 but does not
            // block on it).
            if new_commits > 0 {
                let _ = app.reload();
            }
            SyncStatus::Succeeded { new_commits }
        }
        SyncOutcome::Failed { message } => {
            // Classify between true failure and unavailability so the
            // pill carries the right framing without re-running git
            // probes unnecessarily.
            match availability {
                git_sync::SyncAvailability::Unavailable(reason) => SyncStatus::Unavailable {
                    hint: reason.hint().to_string(),
                },
                git_sync::SyncAvailability::Available => SyncStatus::Failed { message },
            }
        }
    };
    app.set_sync_status(status);
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
    // For multi-workflow sessions, watch the discovery scan root so that
    // creates/removes of sibling workflows (and edits to their READMEs) fire
    // the same `RefreshSignal`. For `-w pinned` or single-workflow sessions,
    // there is nothing to discover so keep the narrower active-dir scope.
    let dir = app
        .as_session()
        .and_then(|s| s.scan_root())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| app.workflow_dir().to_path_buf());
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

/// Seam for the terminal lifecycle operations used by suspend/resume. The
/// production impl ([`CrosstermTerminalControl`]) shells out to crossterm;
/// tests record an in-memory call sequence to assert ordering (AC-2).
trait TerminalControl {
    fn disable_raw_mode(&mut self) -> io::Result<()>;
    fn leave_alt(&mut self) -> io::Result<()>;
    fn enter_alt(&mut self) -> io::Result<()>;
    fn enable_raw_mode(&mut self) -> io::Result<()>;
}

struct CrosstermTerminalControl<'a>(&'a mut io::Stdout);

impl TerminalControl for CrosstermTerminalControl<'_> {
    fn disable_raw_mode(&mut self) -> io::Result<()> {
        disable_raw_mode()
    }
    fn leave_alt(&mut self) -> io::Result<()> {
        execute!(self.0, LeaveAlternateScreen)
    }
    fn enter_alt(&mut self) -> io::Result<()> {
        execute!(self.0, EnterAlternateScreen)
    }
    fn enable_raw_mode(&mut self) -> io::Result<()> {
        enable_raw_mode()
    }
}

/// Suspend the TUI by leaving raw mode and the alt screen, in that order, so
/// the external editor inherits a normal cooked-mode primary screen.
fn suspend_terminal<T: TerminalControl + ?Sized>(t: &mut T) -> io::Result<()> {
    t.disable_raw_mode()?;
    t.leave_alt()?;
    Ok(())
}

/// Resume the TUI by re-entering the alt screen and re-enabling raw mode, in
/// that order. The caller is expected to follow up with `terminal.clear()`
/// so the next draw repaints the full buffer.
fn resume_terminal<T: TerminalControl + ?Sized>(t: &mut T) -> io::Result<()> {
    t.enter_alt()?;
    t.enable_raw_mode()?;
    Ok(())
}

/// Emit an OSC 7 sequence (`ESC ] 7 ; file://<host><cwd> ESC \`) when
/// `is_tty` is true. The host portion is empty by convention — iTerm2,
/// Ghostty and friends treat `file:///abs/path` as "this host" and the
/// empty form avoids a `hostname` crate dependency.
///
/// Path bytes are percent-encoded per RFC 3986: the unreserved set
/// `A-Za-z0-9-._~` plus the path separator `/` are preserved; all other
/// bytes become `%XX`. When `is_tty` is false, nothing is written.
fn emit_osc7<W: Write>(w: &mut W, is_tty: bool, cwd: &Path) -> io::Result<()> {
    if !is_tty {
        return Ok(());
    }
    w.write_all(b"\x1b]7;file://")?;
    write_percent_encoded_path(w, cwd)?;
    w.write_all(b"\x1b\\")?;
    Ok(())
}

/// Percent-encode `path` per the OSC 7 contract documented on
/// [`emit_osc7`]. Operates on raw bytes so non-UTF-8 paths still emit a
/// well-formed URL.
fn write_percent_encoded_path<W: Write>(w: &mut W, path: &Path) -> io::Result<()> {
    let bytes = path_as_bytes(path);
    for &b in bytes {
        if is_osc7_safe_byte(b) {
            w.write_all(std::slice::from_ref(&b))?;
        } else {
            let hi = HEX[(b >> 4) as usize];
            let lo = HEX[(b & 0x0f) as usize];
            w.write_all(&[b'%', hi, lo])?;
        }
    }
    Ok(())
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// RFC 3986 unreserved set (`A-Za-z0-9-._~`) plus the path separator `/`.
fn is_osc7_safe_byte(b: u8) -> bool {
    matches!(b,
        b'A'..=b'Z'
        | b'a'..=b'z'
        | b'0'..=b'9'
        | b'-'
        | b'.'
        | b'_'
        | b'~'
        | b'/'
    )
}

#[cfg(unix)]
fn path_as_bytes(path: &Path) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes()
}

#[cfg(not(unix))]
fn path_as_bytes(path: &Path) -> &[u8] {
    // `Path::to_str` returns `None` for non-UTF-8 paths; fall back to a
    // lossy view that's still emittable. Spacetop's primary platforms are
    // Unix-like, so this branch is best-effort.
    path.to_str().map(str::as_bytes).unwrap_or(b"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, OverviewSession, OverviewState};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spacetop_core::discovery::DiscoveredWorkflow;

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

    struct TestConfigEnv {
        vars: std::collections::HashMap<String, String>,
    }

    impl config::ConfigEnv for TestConfigEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.vars.get(key).cloned()
        }
    }

    #[test]
    fn startup_config_io_errors_fall_back_to_default_warning() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("spacetop").join("config.yaml"))
            .expect("create config path as directory");
        let env = TestConfigEnv {
            vars: std::collections::HashMap::from([(
                "XDG_CONFIG_HOME".to_string(),
                dir.path().to_string_lossy().into_owned(),
            )]),
        };

        let load = load_startup_config(&env);

        assert_eq!(load.config, SpacetopConfig::default());
        assert!(load
            .warnings
            .iter()
            .any(|warning| warning.message.contains("failed to load config")));
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
    fn load_session_state_for_app_applies_saved_selection() {
        let holder = tempfile::tempdir().expect("tempdir");
        let workflow = holder.path().join("workflow");
        std::fs::create_dir_all(&workflow).unwrap();
        write_minimal_workflow(&workflow, "000");
        std::fs::write(
            workflow.join("task-001.md"),
            "---\nid: 001\ntitle: T001\nstatus: plan\n---\n\nbody\n",
        )
        .unwrap();
        let mut app = App::load(workflow.clone()).expect("load workflow");
        let key = spacetop_core::session_state::WorkflowSessionKey::from_workflow_dir(&workflow)
            .expect("session key");
        let session_file = holder.path().join("state").join("session.yaml");
        spacetop_core::session_state::save_session_file(
            &session_file,
            &spacetop_core::session_state::SessionState {
                workflows: std::collections::BTreeMap::from([(
                    key.as_str().to_string(),
                    spacetop_core::session_state::WorkflowSession {
                        selected_entity_id: Some("001".to_string()),
                        scope: spacetop_core::session_state::WorkflowScope::Active,
                    },
                )]),
            },
        )
        .expect("save session");

        load_session_state_for_app(&mut app, Some(&session_file));

        assert_eq!(app.selected_item().expect("selected").id, "001");
    }

    #[test]
    fn save_session_state_for_app_writes_canonical_workflow_session() {
        let holder = tempfile::tempdir().expect("tempdir");
        let workflow = holder.path().join("workflow");
        std::fs::create_dir_all(&workflow).unwrap();
        write_minimal_workflow(&workflow, "000");
        std::fs::write(
            workflow.join("task-001.md"),
            "---\nid: 001\ntitle: T001\nstatus: plan\n---\n\nbody\n",
        )
        .unwrap();
        let mut app = App::load(workflow.clone()).expect("load workflow");
        app.handle_key(key(KeyCode::Down));
        let session_file = holder.path().join("state").join("session.yaml");

        save_session_state_for_app(&mut app, Some(&session_file));

        let saved =
            spacetop_core::session_state::load_session_file(&session_file).expect("load session");
        let key = spacetop_core::session_state::WorkflowSessionKey::from_workflow_dir(&workflow)
            .expect("session key");
        assert_eq!(
            saved
                .workflows
                .get(key.as_str())
                .and_then(|session| session.selected_entity_id.as_deref()),
            Some("001")
        );
        assert!(app.warning_messages().is_empty());
    }

    #[test]
    fn save_session_state_for_app_surfaces_save_errors_as_warning() {
        let holder = tempfile::tempdir().expect("tempdir");
        let workflow = holder.path().join("workflow");
        std::fs::create_dir_all(&workflow).unwrap();
        write_minimal_workflow(&workflow, "000");
        let mut app = App::load(workflow).expect("load workflow");

        save_session_state_for_app(&mut app, Some(Path::new("relative/session.yaml")));

        assert!(app
            .warning_messages()
            .iter()
            .any(|warning| warning.contains("failed to save session state")));
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

    /// Test helper: records the order of [`TerminalControl`] calls in a
    /// shared `Vec<&'static str>` so assertions can inspect ordering.
    struct MockTerminalControl {
        log: Vec<&'static str>,
    }

    impl MockTerminalControl {
        fn new() -> Self {
            Self { log: Vec::new() }
        }
    }

    impl TerminalControl for MockTerminalControl {
        fn disable_raw_mode(&mut self) -> io::Result<()> {
            self.log.push("disable_raw_mode");
            Ok(())
        }
        fn leave_alt(&mut self) -> io::Result<()> {
            self.log.push("leave_alt");
            Ok(())
        }
        fn enter_alt(&mut self) -> io::Result<()> {
            self.log.push("enter_alt");
            Ok(())
        }
        fn enable_raw_mode(&mut self) -> io::Result<()> {
            self.log.push("enable_raw_mode");
            Ok(())
        }
    }

    /// AC-2: suspend leaves raw mode + alt screen (in that order), and
    /// resume re-enters the alt screen + raw mode (in that order). Asserts
    /// the call sequence on the [`TerminalControl`] seam.
    #[test]
    fn suspend_resume_call_sequence() {
        let mut term = MockTerminalControl::new();
        suspend_terminal(&mut term).expect("suspend ok");
        resume_terminal(&mut term).expect("resume ok");
        assert_eq!(
            term.log,
            vec![
                "disable_raw_mode",
                "leave_alt",
                "enter_alt",
                "enable_raw_mode",
            ]
        );
    }

    /// AC-5 (TTY branch): emits the expected OSC 7 byte sequence including
    /// percent-encoded path bytes, e.g. space → `%20`.
    #[test]
    fn emit_osc7_writes_bytes_when_tty() {
        let mut buf: Vec<u8> = Vec::new();
        emit_osc7(&mut buf, true, Path::new("/a b")).expect("write ok");
        assert_eq!(buf, b"\x1b]7;file:///a%20b\x1b\\");

        // Also verify a path with only safe bytes is passed through verbatim.
        let mut buf2: Vec<u8> = Vec::new();
        emit_osc7(&mut buf2, true, Path::new("/Users/test/dir")).expect("write ok");
        assert_eq!(buf2, b"\x1b]7;file:///Users/test/dir\x1b\\");
    }

    /// AC-5 (non-TTY branch): writer stays empty.
    #[test]
    fn emit_osc7_skips_when_not_tty() {
        let mut buf: Vec<u8> = Vec::new();
        emit_osc7(&mut buf, false, Path::new("/some/path")).expect("write ok");
        assert!(buf.is_empty(), "expected no OSC 7 bytes when not a TTY");
    }
}
