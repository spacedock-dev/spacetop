use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};

use spacetop_core::config::{ConfigWarning, SpacetopConfig};
use spacetop_core::discovery::DiscoveredWorkflow;
use spacetop_core::domain::{Entity, WorkflowSnapshot};
use spacetop_core::parser::ParseError;
use spacetop_core::query::EntityQuery;
use spacetop_core::session_state::{SessionState, WorkflowSessionKey};

mod history_worker;
mod keys;
mod mouse;
mod overview;
mod picker;
mod search;
mod session;
mod session_activity_worker;

pub use history_worker::{spawn_history_worker, HistoryWorkerRequest, HistoryWorkerResult};
pub use overview::{
    OverviewState, PreviewPlacement, SelectedRow, SortMode, StageCount, StateTopologyDiagnostic,
    SyncStatus, ViewScope,
};
pub use picker::PickerState;
pub use search::{
    matching_commands, CommandAction, CommandEntry, SearchMode, SearchState,
    SEARCH_VISIBLE_RESULT_LIMIT,
};
pub use session::{OverviewSession, WorkflowSwitch};
pub use session_activity_worker::{
    spawn_session_activity_worker, SessionActivityWorkerRequest, SessionActivityWorkerResult,
};

pub(crate) use keys::ResolvedKeymap;
use keys::{handle_overview_key_with_keymap, OverviewKeyAction};

const COPY_FEEDBACK_DURATION: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CopyFeedback {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TimedCopyFeedback {
    outcome: CopyFeedback,
    expires_at: Instant,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum AppMode {
    Picker(PickerState),
    Overview(OverviewSession),
    /// Picker overlay opened from inside an overview (re-discovery). The
    /// underlying session is preserved so `Esc` restores it verbatim.
    PickerOverlay {
        underlying: OverviewSession,
        picker: PickerState,
    },
    /// Full-pane workflow-definition view scoped to the active workflow.
    /// The underlying overview session is preserved verbatim so `Esc`
    /// restores it including the active tab, selection, scope, sort
    /// mode, and preview state. `scroll` is the body scroll offset in
    /// rows (clamped at render time to the max scroll for the content
    /// height).
    Definition {
        underlying: OverviewSession,
        scroll: usize,
    },
    Search {
        underlying: OverviewSession,
        state: SearchState,
    },
    Timeline {
        underlying: OverviewSession,
        entity_id: String,
        scroll: usize,
    },
    Metrics {
        underlying: OverviewSession,
        scroll: usize,
    },
    Activity {
        underlying: OverviewSession,
        scroll: usize,
    },
    Relations {
        underlying: OverviewSession,
        entity_id: String,
        scroll: usize,
    },
}

impl AppMode {
    pub(crate) fn as_session(&self) -> Option<&OverviewSession> {
        match self {
            Self::Overview(session)
            | Self::PickerOverlay {
                underlying: session,
                ..
            }
            | Self::Definition {
                underlying: session,
                ..
            }
            | Self::Search {
                underlying: session,
                ..
            }
            | Self::Timeline {
                underlying: session,
                ..
            }
            | Self::Metrics {
                underlying: session,
                ..
            }
            | Self::Activity {
                underlying: session,
                ..
            }
            | Self::Relations {
                underlying: session,
                ..
            } => Some(session),
            Self::Picker(_) => None,
        }
    }

    pub(crate) fn as_session_mut(&mut self) -> Option<&mut OverviewSession> {
        match self {
            Self::Overview(session)
            | Self::PickerOverlay {
                underlying: session,
                ..
            }
            | Self::Definition {
                underlying: session,
                ..
            }
            | Self::Search {
                underlying: session,
                ..
            }
            | Self::Timeline {
                underlying: session,
                ..
            }
            | Self::Metrics {
                underlying: session,
                ..
            }
            | Self::Activity {
                underlying: session,
                ..
            }
            | Self::Relations {
                underlying: session,
                ..
            } => Some(session),
            Self::Picker(_) => None,
        }
    }

    pub(crate) fn into_session(self) -> Option<OverviewSession> {
        match self {
            Self::Overview(session)
            | Self::PickerOverlay {
                underlying: session,
                ..
            }
            | Self::Definition {
                underlying: session,
                ..
            }
            | Self::Search {
                underlying: session,
                ..
            }
            | Self::Timeline {
                underlying: session,
                ..
            }
            | Self::Metrics {
                underlying: session,
                ..
            }
            | Self::Activity {
                underlying: session,
                ..
            }
            | Self::Relations {
                underlying: session,
                ..
            } => Some(session),
            Self::Picker(_) => None,
        }
    }
}

fn definition_scroll_down(scroll: &mut usize, rows: usize, max_scroll: usize) {
    let current = (*scroll).min(max_scroll);
    *scroll = current.saturating_add(rows).min(max_scroll);
}

fn definition_scroll_up(scroll: &mut usize, rows: usize, max_scroll: usize) {
    let current = (*scroll).min(max_scroll);
    *scroll = current.saturating_sub(rows);
}

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    mode: AppMode,
    definition_max_scroll: Cell<usize>,
    config: SpacetopConfig,
    config_warnings: Vec<ConfigWarning>,
    resolved_keymap: ResolvedKeymap,
    session_state: SessionState,
    runtime_warnings: Vec<String>,
    should_quit: bool,
    help_open: bool,
    pending_switch: Option<WorkflowSwitch>,
    pending_overlay_open: bool,
    pending_open_file: Option<PathBuf>,
    pending_sync: bool,
    pending_copy_id: Option<String>,
    id_click_candidate: Option<mouse::IdClickCandidate>,
    copy_feedback: Option<TimedCopyFeedback>,
}

impl App {
    pub fn new(workflow_dir: impl Into<PathBuf>) -> Self {
        Self::new_with_config(workflow_dir, SpacetopConfig::default())
    }

    pub fn new_with_config(workflow_dir: impl Into<PathBuf>, config: SpacetopConfig) -> Self {
        Self::new_with_config_warnings(workflow_dir, config, Vec::new())
    }

    pub fn new_with_config_warnings(
        workflow_dir: impl Into<PathBuf>,
        config: SpacetopConfig,
        config_warnings: Vec<ConfigWarning>,
    ) -> Self {
        let mut state = OverviewState::empty(workflow_dir.into());
        state.apply_config_defaults(&config);
        Self::from_mode_with_config(
            AppMode::Overview(OverviewSession::single(state, true)),
            config,
            config_warnings,
        )
    }

    pub fn load(workflow_dir: PathBuf) -> Result<Self, ParseError> {
        Self::load_with_config(workflow_dir, SpacetopConfig::default())
    }

    pub fn load_with_config(
        workflow_dir: PathBuf,
        config: SpacetopConfig,
    ) -> Result<Self, ParseError> {
        Self::load_with_config_warnings(workflow_dir, config, Vec::new())
    }

    pub fn load_with_config_warnings(
        workflow_dir: PathBuf,
        config: SpacetopConfig,
        config_warnings: Vec<ConfigWarning>,
    ) -> Result<Self, ParseError> {
        let mut state = OverviewState::load(workflow_dir)?;
        state.apply_config_defaults(&config);
        Ok(Self::from_mode_with_config(
            AppMode::Overview(OverviewSession::single(state, true)),
            config,
            config_warnings,
        ))
    }

    pub fn from_snapshot(workflow_dir: PathBuf, snapshot: WorkflowSnapshot) -> Self {
        Self::from_snapshot_with_config(workflow_dir, snapshot, SpacetopConfig::default())
    }

    pub fn from_snapshot_with_config(
        workflow_dir: PathBuf,
        snapshot: WorkflowSnapshot,
        config: SpacetopConfig,
    ) -> Self {
        Self::from_snapshot_with_config_warnings(workflow_dir, snapshot, config, Vec::new())
    }

    pub fn from_snapshot_with_config_warnings(
        workflow_dir: PathBuf,
        snapshot: WorkflowSnapshot,
        config: SpacetopConfig,
        config_warnings: Vec<ConfigWarning>,
    ) -> Self {
        let mut state = OverviewState::from_snapshot(workflow_dir, snapshot);
        state.apply_config_defaults(&config);
        Self::from_mode_with_config(
            AppMode::Overview(OverviewSession::single(state, true)),
            config,
            config_warnings,
        )
    }

    pub fn from_session(session: OverviewSession) -> Self {
        Self::from_session_with_config(session, SpacetopConfig::default())
    }

    pub fn from_session_with_config(session: OverviewSession, config: SpacetopConfig) -> Self {
        Self::from_session_with_config_warnings(session, config, Vec::new())
    }

    pub fn from_session_with_config_warnings(
        session: OverviewSession,
        config: SpacetopConfig,
        config_warnings: Vec<ConfigWarning>,
    ) -> Self {
        Self::from_mode_with_config(AppMode::Overview(session), config, config_warnings)
    }

    pub fn from_picker(scan_root: PathBuf, workflows: Vec<DiscoveredWorkflow>) -> Self {
        Self::from_picker_with_config(scan_root, workflows, SpacetopConfig::default())
    }

    pub fn from_picker_with_config(
        scan_root: PathBuf,
        workflows: Vec<DiscoveredWorkflow>,
        config: SpacetopConfig,
    ) -> Self {
        Self::from_picker_with_config_warnings(scan_root, workflows, config, Vec::new())
    }

    pub fn from_picker_with_config_warnings(
        scan_root: PathBuf,
        workflows: Vec<DiscoveredWorkflow>,
        config: SpacetopConfig,
        config_warnings: Vec<ConfigWarning>,
    ) -> Self {
        debug_assert!(
            workflows.len() >= 2,
            "picker mode requires at least two workflows"
        );
        Self::from_mode_with_config(
            AppMode::Picker(PickerState::new(scan_root, workflows)),
            config,
            config_warnings,
        )
    }

    fn from_mode_with_config(
        mode: AppMode,
        config: SpacetopConfig,
        config_warnings: Vec<ConfigWarning>,
    ) -> Self {
        let resolved_keymap = ResolvedKeymap::from_config(&config);
        Self {
            mode,
            definition_max_scroll: Cell::new(usize::MAX),
            config,
            config_warnings,
            resolved_keymap,
            session_state: SessionState::default(),
            runtime_warnings: Vec::new(),
            should_quit: false,
            help_open: false,
            pending_switch: None,
            pending_overlay_open: false,
            pending_open_file: None,
            pending_sync: false,
            pending_copy_id: None,
            id_click_candidate: None,
            copy_feedback: None,
        }
    }

    pub fn config(&self) -> &SpacetopConfig {
        &self.config
    }

    pub fn config_warnings(&self) -> &[ConfigWarning] {
        &self.config_warnings
    }

    pub(crate) fn keymap(&self) -> &ResolvedKeymap {
        &self.resolved_keymap
    }

    pub fn keymap_warnings(&self) -> &[String] {
        self.resolved_keymap.warnings()
    }

    pub(crate) fn warning_messages(&self) -> Vec<String> {
        self.config_warnings
            .iter()
            .map(|warning| warning.message.clone())
            .chain(self.resolved_keymap.warnings().iter().cloned())
            .chain(self.runtime_warnings.iter().cloned())
            .collect()
    }

    pub fn add_status_warning(&mut self, message: String) {
        self.runtime_warnings.push(message);
    }

    pub fn help_open(&self) -> bool {
        self.help_open
    }

    pub fn toggle_help(&mut self) {
        self.help_open = !self.help_open;
    }

    pub fn close_help(&mut self) {
        self.help_open = false;
    }

    pub fn mode(&self) -> &AppMode {
        &self.mode
    }

    pub fn as_overview(&self) -> Option<&OverviewState> {
        match &self.mode {
            AppMode::Overview(session) => Some(session.active_state()),
            _ => None,
        }
    }

    pub fn as_session(&self) -> Option<&OverviewSession> {
        self.mode.as_session()
    }

    pub fn apply_session_state(&mut self, session_state: SessionState) {
        self.session_state = session_state;
        if let Some(session) = self.mode.as_session_mut() {
            session.apply_session_state(&self.session_state);
        }
    }

    pub fn session_state_snapshot(&self) -> SessionState {
        let mut session_state = self.session_state.clone();
        if let Some(session) = self.mode.as_session() {
            session.write_session_state(&mut session_state);
        }
        session_state
    }

    /// True while the definition view is active.
    pub fn is_definition(&self) -> bool {
        matches!(self.mode, AppMode::Definition { .. })
    }

    /// Body scroll offset of the definition view. Returns `None` when the
    /// definition view is not active.
    pub fn definition_scroll(&self) -> Option<usize> {
        match &self.mode {
            AppMode::Definition { scroll, .. } => Some(*scroll),
            _ => None,
        }
    }

    pub(crate) fn set_definition_max_scroll(&self, max_scroll: usize) {
        self.definition_max_scroll.set(max_scroll);
    }

    pub fn as_picker(&self) -> Option<&PickerState> {
        match &self.mode {
            AppMode::Picker(state) => Some(state),
            AppMode::PickerOverlay { picker, .. } => Some(picker),
            _ => None,
        }
    }

    pub fn is_overlay(&self) -> bool {
        matches!(self.mode, AppMode::PickerOverlay { .. })
    }

    /// Drain any pending workflow switch. The event loop calls this each
    /// frame after dispatching a key and runs watcher teardown + load/reload
    /// + watcher restart synchronously.
    pub fn take_pending_switch(&mut self) -> Option<WorkflowSwitch> {
        self.pending_switch.take()
    }

    /// Drain a pending picker-overlay open request. The event loop runs
    /// `discovery::discover_workflows` against `session.scan_root()` and then
    /// calls [`App::open_picker_overlay_with`].
    pub fn take_pending_overlay_open(&mut self) -> bool {
        std::mem::replace(&mut self.pending_overlay_open, false)
    }

    /// Drain any pending "open file in $EDITOR" intent recorded by the `o`
    /// keybind. The event loop suspends the TUI, blocks on the editor, then
    /// resumes — the actual I/O lives in `run_terminal`, not on `App`.
    pub fn take_pending_open_file(&mut self) -> Option<PathBuf> {
        self.pending_open_file.take()
    }

    /// Drain the full entity ID queued by a mouse double-click. OSC 52 output
    /// remains in the terminal event loop rather than app/input state.
    pub fn take_pending_copy_id(&mut self) -> Option<String> {
        self.pending_copy_id.take()
    }

    pub(crate) fn set_copy_feedback_at(&mut self, outcome: CopyFeedback, now: Instant) {
        self.copy_feedback = Some(TimedCopyFeedback {
            outcome,
            expires_at: now + COPY_FEEDBACK_DURATION,
        });
    }

    pub(crate) fn copy_feedback(&self) -> Option<CopyFeedback> {
        self.copy_feedback_at(Instant::now())
    }

    pub(crate) fn copy_feedback_at(&self, now: Instant) -> Option<CopyFeedback> {
        self.copy_feedback
            .as_ref()
            .filter(|feedback| now < feedback.expires_at)
            .map(|feedback| feedback.outcome)
    }

    /// Record a `Y` keypress intent. The event loop calls
    /// `take_pending_sync` next tick and runs `git_sync::sync` against the
    /// active workflow's repo root, synchronously.
    pub fn request_sync(&mut self) {
        self.pending_sync = true;
    }

    /// Drain any pending sync request. Returns `true` exactly once per
    /// `request_sync` call.
    pub fn take_pending_sync(&mut self) -> bool {
        std::mem::replace(&mut self.pending_sync, false)
    }

    /// Current sync status for the active overview tab, if any.
    pub fn sync_status(&self) -> Option<&SyncStatus> {
        self.as_session()
            .and_then(|s| s.active_state().sync_status())
    }

    /// Set the sync status pill on the active overview tab. Used by the
    /// event loop after running the git_sync helper.
    pub fn set_sync_status(&mut self, status: SyncStatus) {
        if let Some(session) = self.mode.as_session_mut() {
            session.active_state_mut().set_sync_status(status);
        }
    }

    pub fn history_worker_request(&self) -> Option<HistoryWorkerRequest> {
        self.mode
            .as_session()
            .and_then(|session| session.active_state().history_worker_request())
    }

    pub fn apply_history_result(&mut self, result: HistoryWorkerResult) {
        if let Some(session) = self.mode.as_session_mut() {
            session.active_state_mut().apply_history_result(result);
        }
    }

    pub fn session_activity_worker_request(&self) -> Option<SessionActivityWorkerRequest> {
        self.mode
            .as_session()
            .and_then(|session| session.active_state().session_activity_worker_request())
    }

    pub fn apply_session_activity_result(&mut self, result: SessionActivityWorkerResult) {
        if let Some(session) = self.mode.as_session_mut() {
            session
                .active_state_mut()
                .apply_session_activity_result(result);
        }
    }

    /// Repo root of the active workflow (the git ancestor the discovery
    /// scan resolved against). Used by the event-loop drain to target
    /// `git pull` at the right directory.
    pub fn repo_root(&self) -> Option<&Path> {
        self.as_session()
            .map(|s| s.active_state().repo_root.as_path())
    }

    pub fn workflow_storage(&self) -> Option<&spacetop_core::domain::WorkflowStorage> {
        self.as_session()
            .map(|session| session.active_state().storage())
    }

    /// Open a picker overlay with the given (possibly re-discovered)
    /// workflow list. If `result` is an `Err`, the overlay still opens with
    /// the prior session's discovery list and the error string is surfaced.
    pub fn open_picker_overlay_with(&mut self, result: Result<Vec<DiscoveredWorkflow>, String>) {
        // Take the current session out of the mode (we're transitioning to
        // PickerOverlay).
        let prior_mode = std::mem::replace(
            &mut self.mode,
            AppMode::Picker(PickerState::new(PathBuf::new(), Vec::new())),
        );
        let session = match prior_mode {
            AppMode::Picker(state) => {
                self.mode = AppMode::Picker(state);
                return;
            }
            other => other
                .into_session()
                .expect("session-backed modes must open picker overlay"),
        };
        // Build picker state. Use scan_root if known; fallback to active dir.
        let scan_root = session
            .scan_root()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| session.active_dir().to_path_buf());
        let (workflows, err) = match result {
            Ok(list) => (list, None),
            Err(e) => (session.discovery().to_vec(), Some(e)),
        };
        // Pre-select the active workflow's path if it is still present in
        // the new list; otherwise fall back to 0.
        let active_root = session.active_dir().to_path_buf();
        let preselect = workflows
            .iter()
            .position(|d| d.root == active_root)
            .unwrap_or(0);
        let mut picker = PickerState::new(scan_root, workflows);
        picker.selected_index = preselect;
        if let Some(msg) = err {
            picker.set_error(msg);
        }
        self.mode = AppMode::PickerOverlay {
            underlying: session,
            picker,
        };
    }

    // --- Back-compat accessors so existing overview tests keep compiling. ---

    fn overview(&self) -> &OverviewState {
        self.mode
            .as_session()
            .expect("called overview accessor while in picker mode")
            .active_state()
    }

    pub fn workflow_dir(&self) -> &Path {
        self.overview().workflow_dir()
    }

    pub fn snapshot(&self) -> WorkflowSnapshot {
        self.overview().snapshot()
    }

    pub fn selected_index(&self) -> usize {
        self.overview().selected_index()
    }

    pub fn selected_item(&self) -> Option<Entity> {
        self.overview().selected_item()
    }

    pub fn stage_counts(&self) -> Vec<StageCount> {
        self.overview().stage_counts()
    }

    pub fn view_scope(&self) -> ViewScope {
        self.overview().view_scope()
    }

    pub fn visible_items(&self) -> Vec<Entity> {
        self.overview().visible_items()
    }

    pub fn archived_items(&self) -> Vec<Entity> {
        self.overview().archived_items()
    }

    pub fn archived_count(&self) -> Option<usize> {
        self.overview().archived_count()
    }

    pub fn archive_error(&self) -> Option<&str> {
        self.overview().archive_error()
    }

    pub fn last_refresh_error(&self) -> Option<&str> {
        self.mode
            .as_session()
            .and_then(|session| session.active_state().last_refresh_error())
    }

    pub fn set_refresh_error(&mut self, message: String) {
        if let Some(session) = self.mode.as_session_mut() {
            session.active_state_mut().set_refresh_error(message);
        }
    }

    pub fn reload_from_snapshot(&mut self, snapshot: WorkflowSnapshot) {
        if let Some(session) = self.mode.as_session_mut() {
            session.active_state_mut().reload_from_snapshot(snapshot);
        }
    }

    pub fn reload(&mut self) -> Result<(), ParseError> {
        match self.mode.as_session_mut() {
            Some(session) => session.active_state_mut().reload(),
            None => Ok(()),
        }
    }

    /// Watcher-driven reload that also re-runs discovery when the session has
    /// a scan root. Used by the event loop when a `RefreshSignal` arrives so
    /// that a workflow added or removed under the discovery root becomes
    /// visible, and an edit to the active workflow's README re-parses live.
    ///
    /// Order: discovery first (so the active slot's container is up to date),
    /// then per-active reload. If the active workflow's directory has
    /// disappeared, the active slot is replaced with an empty state carrying
    /// a "workflow removed" message in `last_refresh_error` so the UI stays
    /// non-panicking until the user picks another workflow.
    pub fn reload_with_rediscovery(&mut self) -> Result<(), ParseError> {
        let config = self.config.clone();
        let session_state = self.session_state.clone();
        let Some(session) = self.mode.as_session_mut() else {
            return Ok(());
        };

        let prior_active_dir = session.active_dir().to_path_buf();

        if let Some(scan_root) = session.scan_root().map(|p| p.to_path_buf()) {
            match spacetop_core::discovery::discover_workflows(&scan_root) {
                Ok(new_discovery) if new_discovery.is_empty() => {
                    // The last workflow under the scan root was removed.
                    // Don't replace the discovery list (that would leave the
                    // session structurally empty); instead, swap the active
                    // slot for an empty state with a clear message.
                    let mut empty = OverviewState::empty(prior_active_dir);
                    empty.set_refresh_error("workflow removed".to_string());
                    session.install_active_state(empty);
                    return Ok(());
                }
                Ok(new_discovery) => {
                    session.replace_discovery(new_discovery);
                }
                Err(err) => {
                    session
                        .active_state_mut()
                        .set_refresh_error(format!("re-discovery failed: {err}"));
                }
            }
        }

        let active_dir = session.active_dir().to_path_buf();
        if !active_dir.exists() {
            let mut empty = OverviewState::empty(active_dir);
            empty.set_refresh_error("workflow removed".to_string());
            session.install_active_state(empty);
            return Ok(());
        }

        // If the active slot has not been materialized yet (e.g. discovery
        // remap picked a never-loaded workflow), load it now. Otherwise call
        // the in-place reload, which already preserves prior good state on
        // parse failure and records the error in `last_refresh_error`.
        if !session.active_slot_loaded() {
            match OverviewState::load(active_dir.clone()) {
                Ok(mut state) => {
                    apply_config_and_session_state(&mut state, &config, &session_state);
                    session.install_active_state(state);
                    Ok(())
                }
                Err(err) => {
                    let mut empty = OverviewState::empty(active_dir);
                    empty.set_refresh_error(err.to_string());
                    session.install_active_state(empty);
                    Err(err)
                }
            }
        } else {
            session.active_state_mut().reload()
        }
    }

    /// Materialize the active slot of the current session by loading from
    /// disk. Used by the event loop after a `WorkflowSwitch` with
    /// `needs_first_load == true`. On parse failure, installs a synthetic
    /// empty `OverviewState` with `last_refresh_error` set so the user sees
    /// the breadcrumb and the error rather than a hang or silent revert.
    pub fn materialize_active(&mut self) {
        let config = self.config.clone();
        let session_state = self.session_state.clone();
        let Some(session) = self.mode.as_session_mut() else {
            return;
        };
        let dir = session.active_dir().to_path_buf();
        match OverviewState::load(dir.clone()) {
            Ok(mut state) => {
                apply_config_and_session_state(&mut state, &config, &session_state);
                session.install_active_state(state);
            }
            Err(err) => {
                let mut empty = OverviewState::empty(dir);
                empty.set_refresh_error(err.to_string());
                session.install_active_state(empty);
            }
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.consume_help_key(key) {
            return;
        }
        let overview_action = match &mut self.mode {
            AppMode::Overview(session) => Some(handle_overview_key_with_keymap(
                session,
                key,
                &self.resolved_keymap,
            )),
            _ => None,
        };
        if let Some(action) = overview_action {
            self.apply_overview_key_action(action);
            return;
        }

        // Overlay confirm needs `&mut self` (it rebuilds the mode), so it
        // is intercepted ahead of the per-mode match below and shared with
        // the mouse click path (AC-5).
        if matches!(self.mode, AppMode::PickerOverlay { .. }) && key.code == KeyCode::Enter {
            self.confirm_picker_overlay();
            return;
        }

        let definition_max_scroll = self.definition_max_scroll.get();
        match &mut self.mode {
            AppMode::Overview(_) => {}
            AppMode::Picker(state) => match key.code {
                KeyCode::Char('?') => self.help_open = true,
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Down | KeyCode::Char('j') => state.select_next(),
                KeyCode::Up | KeyCode::Char('k') => state.select_previous(),
                KeyCode::PageDown => state.page_selection_down(),
                KeyCode::PageUp => state.page_selection_up(),
                KeyCode::Home => state.select_first(),
                KeyCode::End => state.select_last(),
                KeyCode::Enter => {
                    if let Some(next_mode) = picker_enter_transition(state) {
                        self.mode = next_mode;
                    }
                }
                _ => {}
            },
            AppMode::Definition { underlying, scroll } => match key.code {
                KeyCode::Char('?') => self.help_open = true,
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('D') => {
                    let restored = std::mem::replace(
                        underlying,
                        OverviewSession::single(OverviewState::empty(PathBuf::new()), true),
                    );
                    self.mode = AppMode::Overview(restored);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    definition_scroll_down(scroll, 1, definition_max_scroll);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    definition_scroll_up(scroll, 1, definition_max_scroll);
                }
                KeyCode::PageDown => {
                    definition_scroll_down(scroll, 10, definition_max_scroll);
                }
                KeyCode::PageUp => {
                    definition_scroll_up(scroll, 10, definition_max_scroll);
                }
                KeyCode::Home => {
                    *scroll = 0;
                }
                KeyCode::End => {
                    *scroll = usize::MAX;
                }
                _ => {}
            },
            AppMode::Search { .. } => {
                self.handle_search_key(key);
            }
            AppMode::Timeline {
                underlying, scroll, ..
            }
            | AppMode::Metrics { underlying, scroll }
            | AppMode::Activity { underlying, scroll }
            | AppMode::Relations {
                underlying, scroll, ..
            } => match key.code {
                KeyCode::Char('?') => self.help_open = true,
                KeyCode::Esc | KeyCode::Char('q') => {
                    let restored = std::mem::replace(
                        underlying,
                        OverviewSession::single(OverviewState::empty(PathBuf::new()), true),
                    );
                    self.mode = AppMode::Overview(restored);
                }
                KeyCode::Right if underlying.is_multi() => {
                    self.pending_switch = Some(underlying.cycle_next());
                }
                KeyCode::Left if underlying.is_multi() => {
                    self.pending_switch = Some(underlying.cycle_prev());
                }
                KeyCode::Char('P') if underlying.is_multi() && !underlying.pinned_single() => {
                    self.pending_overlay_open = true;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    *scroll = scroll.saturating_add(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    *scroll = scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    *scroll = scroll.saturating_add(10);
                }
                KeyCode::PageUp => {
                    *scroll = scroll.saturating_sub(10);
                }
                KeyCode::Home => {
                    *scroll = 0;
                }
                KeyCode::End => {
                    *scroll = usize::MAX;
                }
                _ => {}
            },
            AppMode::PickerOverlay { underlying, picker } => match key.code {
                KeyCode::Char('?') => self.help_open = true,
                KeyCode::Esc | KeyCode::Char('q') => {
                    // Restore the underlying session — discard the picker.
                    let restored = std::mem::replace(
                        underlying,
                        OverviewSession::single(OverviewState::empty(PathBuf::new()), true),
                    );
                    self.mode = AppMode::Overview(restored);
                }
                KeyCode::Down | KeyCode::Char('j') => picker.select_next(),
                KeyCode::Up | KeyCode::Char('k') => picker.select_previous(),
                KeyCode::PageDown => picker.page_selection_down(),
                KeyCode::PageUp => picker.page_selection_up(),
                KeyCode::Home => picker.select_first(),
                KeyCode::End => picker.select_last(),
                _ => {}
            },
        }
    }

    /// Confirm the currently selected workflow in the picker overlay:
    /// apply the (possibly re-discovered) workflow list into the
    /// underlying session, queue the switch, and return to Overview.
    /// Shared by the Enter key and the mouse click (AC-5) so both confirm
    /// paths stay one transition. No-op when not in overlay mode or when
    /// nothing is selected.
    fn confirm_picker_overlay(&mut self) {
        let AppMode::PickerOverlay { picker, .. } = &mut self.mode else {
            return;
        };
        let Some(selected) = picker.selected().cloned() else {
            return;
        };
        picker.clear_error();
        // Pull session and picker out of mode so we can rebuild.
        let placeholder = AppMode::Picker(PickerState::new(PathBuf::new(), Vec::new()));
        let prior_mode = std::mem::replace(&mut self.mode, placeholder);
        let (mut session, picker_state) = match prior_mode {
            AppMode::PickerOverlay { underlying, picker } => (underlying, picker),
            other => {
                self.mode = other;
                return;
            }
        };
        // Apply discovery list from the picker into the session, then
        // select the chosen workflow.
        let new_workflows = picker_state.workflows().to_vec();
        session.replace_discovery(new_workflows);
        let target_idx = session
            .discovery()
            .iter()
            .position(|d| d.root == selected.root)
            .unwrap_or(0);
        let switch = session.select(target_idx);
        self.pending_switch = Some(switch);
        self.mode = AppMode::Overview(session);
    }

    /// Mouse-event peer to [`App::handle_key`]. Inert while the help popup
    /// is open. Overview and picker modes keep their own hit-testing paths;
    /// the full-pane Definition view handles only wheel scrolling, while
    /// Search, Timeline, Metrics, Activity, and Relations remain inert.
    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        self.handle_mouse_at(mouse, Instant::now());
    }

    pub(crate) fn handle_mouse_at(&mut self, mouse: MouseEvent, now: Instant) {
        if self.help_open {
            return;
        }
        if matches!(
            self.mode,
            AppMode::Picker(_) | AppMode::PickerOverlay { .. }
        ) {
            self.id_click_candidate = None;
            self.handle_picker_mouse(mouse);
            return;
        }
        let definition_max_scroll = self.definition_max_scroll.get();
        let action = match &mut self.mode {
            AppMode::Overview(session) => {
                mouse::handle_overview_mouse(session, mouse, now, &mut self.id_click_candidate)
            }
            AppMode::Definition { scroll, .. } => {
                self.id_click_candidate = None;
                match mouse.kind {
                    crossterm::event::MouseEventKind::ScrollDown => {
                        definition_scroll_down(
                            scroll,
                            mouse::WHEEL_SCROLL_ROWS as usize,
                            definition_max_scroll,
                        );
                    }
                    crossterm::event::MouseEventKind::ScrollUp => {
                        definition_scroll_up(
                            scroll,
                            mouse::WHEEL_SCROLL_ROWS as usize,
                            definition_max_scroll,
                        );
                    }
                    _ => {}
                }
                return;
            }
            _ => {
                self.id_click_candidate = None;
                return;
            }
        };
        self.apply_overview_key_action(action);
    }

    /// AC-5: a single left-click on a picker workflow row selects and
    /// confirms it — the same one-action convention as the overview list.
    /// Clicks elsewhere in the dialog (title, footer, blank space) change
    /// nothing.
    fn handle_picker_mouse(&mut self, mouse: MouseEvent) {
        if !matches!(
            mouse.kind,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        ) {
            return;
        }
        let Some(index) = self
            .as_picker()
            .and_then(|picker| mouse::picker_row_at(picker, mouse.column, mouse.row))
        else {
            return;
        };
        match &mut self.mode {
            AppMode::Picker(state) => {
                state.selected_index = index;
                if let Some(next_mode) = picker_enter_transition(state) {
                    self.mode = next_mode;
                }
            }
            AppMode::PickerOverlay { picker, .. } => {
                picker.selected_index = index;
                self.confirm_picker_overlay();
            }
            _ => {}
        }
    }

    fn consume_help_key(&mut self, key: KeyEvent) -> bool {
        if !self.help_open {
            return false;
        }
        if matches!(key.code, KeyCode::Char('?') | KeyCode::Esc) {
            self.help_open = false;
        }
        true
    }

    fn apply_overview_key_action(&mut self, action: OverviewKeyAction) {
        match action {
            OverviewKeyAction::None => {}
            OverviewKeyAction::OpenHelp => self.help_open = true,
            OverviewKeyAction::Quit => self.should_quit = true,
            OverviewKeyAction::Switch(workflow_switch) => {
                self.id_click_candidate = None;
                self.pending_switch = Some(workflow_switch);
            }
            OverviewKeyAction::OpenPickerOverlay => {
                self.id_click_candidate = None;
                self.pending_overlay_open = true;
            }
            OverviewKeyAction::OpenSelectedFile(path) => {
                self.pending_open_file = Some(path);
            }
            OverviewKeyAction::OpenDefinition => self.open_definition(),
            OverviewKeyAction::OpenSearch => self.open_search(SearchMode::Search),
            OverviewKeyAction::OpenCommandPalette => self.open_search(SearchMode::Command),
            OverviewKeyAction::OpenTimeline => self.open_timeline(),
            OverviewKeyAction::OpenMetrics => self.open_metrics(),
            OverviewKeyAction::OpenActivity => self.open_activity(),
            OverviewKeyAction::OpenRelations => self.open_relations(),
            OverviewKeyAction::CopyId(id) => self.pending_copy_id = Some(id),
            OverviewKeyAction::RequestSync => self.pending_sync = true,
        }
    }

    /// Transition `AppMode::Overview(session)` → `AppMode::Definition`,
    /// stashing the underlying session verbatim so `Esc` can restore it.
    fn open_definition(&mut self) {
        let session = match std::mem::replace(
            &mut self.mode,
            AppMode::Picker(PickerState::new(PathBuf::new(), Vec::new())),
        ) {
            AppMode::Overview(s) => s,
            other => {
                // We were not in Overview — restore the prior mode and bail.
                self.mode = other;
                return;
            }
        };
        self.mode = AppMode::Definition {
            underlying: session,
            scroll: 0,
        };
    }

    fn open_search(&mut self, mode: SearchMode) {
        let Some(session) = self.take_overview_session() else {
            return;
        };
        self.mode = AppMode::Search {
            underlying: session,
            state: SearchState::new(mode),
        };
    }

    fn open_timeline(&mut self) {
        let Some(session) = self.take_overview_session() else {
            return;
        };
        self.mode = timeline_mode_or_overview(session);
    }

    fn open_metrics(&mut self) {
        let Some(session) = self.take_overview_session() else {
            return;
        };
        self.mode = AppMode::Metrics {
            underlying: session,
            scroll: 0,
        };
    }

    fn open_activity(&mut self) {
        let Some(session) = self.take_overview_session() else {
            return;
        };
        self.mode = AppMode::Activity {
            underlying: session,
            scroll: 0,
        };
    }

    fn open_relations(&mut self) {
        let Some(session) = self.take_overview_session() else {
            return;
        };
        self.mode = relations_mode_or_overview(session);
    }

    fn take_overview_session(&mut self) -> Option<OverviewSession> {
        let placeholder = AppMode::Picker(PickerState::new(PathBuf::new(), Vec::new()));
        match std::mem::replace(&mut self.mode, placeholder) {
            AppMode::Overview(session) => Some(session),
            other => {
                self.mode = other;
                None
            }
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        let placeholder = AppMode::Picker(PickerState::new(PathBuf::new(), Vec::new()));
        let prior = std::mem::replace(&mut self.mode, placeholder);
        let AppMode::Search {
            underlying,
            mut state,
        } = prior
        else {
            self.mode = prior;
            return;
        };

        self.mode = match key.code {
            KeyCode::Esc => AppMode::Overview(
                AppMode::Search { underlying, state }
                    .into_session()
                    .expect("search mode stores an overview session"),
            ),
            KeyCode::Backspace => {
                state.backspace();
                AppMode::Search { underlying, state }
            }
            KeyCode::Char('?') => {
                self.help_open = true;
                AppMode::Search { underlying, state }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = search_result_len(&underlying, &state);
                state.select_next(len);
                AppMode::Search { underlying, state }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                state.select_previous();
                AppMode::Search { underlying, state }
            }
            KeyCode::Enter => activate_search(underlying, state),
            KeyCode::Char(ch) => {
                state.push(ch);
                AppMode::Search { underlying, state }
            }
            _ => AppMode::Search { underlying, state },
        };
    }
}

fn apply_config_and_session_state(
    state: &mut OverviewState,
    config: &SpacetopConfig,
    session_state: &SessionState,
) {
    state.apply_config_defaults(config);
    let Ok(key) = WorkflowSessionKey::from_workflow_dir(state.workflow_dir()) else {
        return;
    };
    if let Some(saved) = session_state.workflows.get(key.as_str()) {
        state.apply_session(saved);
    }
}

fn search_result_len(session: &OverviewSession, state: &SearchState) -> usize {
    let len = match state.mode() {
        SearchMode::Search => search_results(session, state).len(),
        SearchMode::Command => matching_commands(state.query()).len(),
    };
    len.min(SEARCH_VISIBLE_RESULT_LIMIT)
}

fn search_results(session: &OverviewSession, state: &SearchState) -> Vec<Entity> {
    let active = session.active_state();
    active.index().query(EntityQuery {
        scope: active.current_query_scope(),
        text: Some(state.query().to_string()),
        ..EntityQuery::default()
    })
}

fn activate_search(mut session: OverviewSession, state: SearchState) -> AppMode {
    match state.mode() {
        SearchMode::Search => {
            if let Some(entity) = search_results(&session, &state)
                .into_iter()
                .take(SEARCH_VISIBLE_RESULT_LIMIT)
                .nth(state.selected_index())
            {
                session
                    .active_state_mut()
                    .select_visible_entity_by_id(&entity.id);
            }
            AppMode::Overview(session)
        }
        SearchMode::Command => {
            let Some(command) = matching_commands(state.query())
                .into_iter()
                .take(SEARCH_VISIBLE_RESULT_LIMIT)
                .nth(state.selected_index())
            else {
                return AppMode::Search {
                    underlying: session,
                    state,
                };
            };
            command_mode_or_overview(session, command.action)
        }
    }
}

fn command_mode_or_overview(session: OverviewSession, action: CommandAction) -> AppMode {
    match action {
        CommandAction::Metrics => AppMode::Metrics {
            underlying: session,
            scroll: 0,
        },
        CommandAction::Activity => AppMode::Activity {
            underlying: session,
            scroll: 0,
        },
        CommandAction::Timeline => timeline_mode_or_overview(session),
        CommandAction::Relations => relations_mode_or_overview(session),
    }
}

fn timeline_mode_or_overview(session: OverviewSession) -> AppMode {
    let Some(entity_id) = session
        .active_state()
        .selected_item()
        .map(|entity| entity.id)
    else {
        return AppMode::Overview(session);
    };
    AppMode::Timeline {
        underlying: session,
        entity_id,
        scroll: 0,
    }
}

fn relations_mode_or_overview(session: OverviewSession) -> AppMode {
    let Some(entity_id) = session
        .active_state()
        .selected_item()
        .map(|entity| entity.id)
    else {
        return AppMode::Overview(session);
    };
    AppMode::Relations {
        underlying: session,
        entity_id,
        scroll: 0,
    }
}

fn picker_enter_transition(state: &mut PickerState) -> Option<AppMode> {
    let selected = state.selected().cloned()?;
    state.clear_error();
    let scan_root = state.scan_root().to_path_buf();
    let workflows = state.workflows().to_vec();
    let initial_active = state.selected_index();
    match OverviewState::load(selected.root.clone()) {
        Ok(overview) => {
            let session =
                OverviewSession::from_discovery(scan_root, workflows, initial_active, overview);
            Some(AppMode::Overview(session))
        }
        Err(err) => {
            state.set_error(format!("failed to load {}: {err}", selected.root.display()));
            None
        }
    }
}

#[cfg(test)]
mod tests;
