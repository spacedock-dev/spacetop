use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};

use crate::discovery::DiscoveredWorkflow;
use crate::domain::{WorkItem, WorkflowSnapshot};
use crate::parser::ParseError;

mod keys;
mod overview;
mod picker;
mod session;

pub use overview::{OverviewState, StageCount, ViewScope};
pub use picker::PickerState;
pub use session::{OverviewSession, WorkflowSwitch};

use keys::{handle_overview_key, OverviewKeyAction};

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
}

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    mode: AppMode,
    should_quit: bool,
    help_open: bool,
    pending_switch: Option<WorkflowSwitch>,
    pending_overlay_open: bool,
}

impl App {
    pub fn new(workflow_dir: impl Into<PathBuf>) -> Self {
        let state = OverviewState::empty(workflow_dir.into());
        Self {
            mode: AppMode::Overview(OverviewSession::single(state, true)),
            should_quit: false,
            help_open: false,
            pending_switch: None,
            pending_overlay_open: false,
        }
    }

    pub fn load(workflow_dir: PathBuf) -> Result<Self, ParseError> {
        let state = OverviewState::load(workflow_dir)?;
        Ok(Self {
            mode: AppMode::Overview(OverviewSession::single(state, true)),
            should_quit: false,
            help_open: false,
            pending_switch: None,
            pending_overlay_open: false,
        })
    }

    pub fn from_snapshot(workflow_dir: PathBuf, snapshot: WorkflowSnapshot) -> Self {
        let state = OverviewState::from_snapshot(workflow_dir, snapshot);
        Self {
            mode: AppMode::Overview(OverviewSession::single(state, true)),
            should_quit: false,
            help_open: false,
            pending_switch: None,
            pending_overlay_open: false,
        }
    }

    pub fn from_session(session: OverviewSession) -> Self {
        Self {
            mode: AppMode::Overview(session),
            should_quit: false,
            help_open: false,
            pending_switch: None,
            pending_overlay_open: false,
        }
    }

    pub fn from_picker(scan_root: PathBuf, workflows: Vec<DiscoveredWorkflow>) -> Self {
        debug_assert!(
            workflows.len() >= 2,
            "picker mode requires at least two workflows"
        );
        Self {
            mode: AppMode::Picker(PickerState::new(scan_root, workflows)),
            should_quit: false,
            help_open: false,
            pending_switch: None,
            pending_overlay_open: false,
        }
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
        match &self.mode {
            AppMode::Overview(session) => Some(session),
            AppMode::PickerOverlay { underlying, .. } => Some(underlying),
            _ => None,
        }
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

    /// Open a picker overlay with the given (possibly re-discovered)
    /// workflow list. If `result` is an `Err`, the overlay still opens with
    /// the prior session's discovery list and the error string is surfaced.
    pub fn open_picker_overlay_with(&mut self, result: Result<Vec<DiscoveredWorkflow>, String>) {
        // Take the current session out of the mode (we're transitioning to
        // PickerOverlay).
        let session = match std::mem::replace(
            &mut self.mode,
            AppMode::Picker(PickerState::new(PathBuf::new(), Vec::new())),
        ) {
            AppMode::Overview(s) => s,
            // If we somehow got called from a non-overview state, restore
            // and bail.
            other => {
                self.mode = other;
                return;
            }
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
        match &self.mode {
            AppMode::Overview(session) => session.active_state(),
            AppMode::PickerOverlay { underlying, .. } => underlying.active_state(),
            AppMode::Picker(_) => panic!("called overview accessor while in picker mode"),
        }
    }

    pub fn workflow_dir(&self) -> &Path {
        self.overview().workflow_dir()
    }

    pub fn snapshot(&self) -> &WorkflowSnapshot {
        self.overview().snapshot()
    }

    pub fn selected_index(&self) -> usize {
        self.overview().selected_index()
    }

    pub fn selected_item(&self) -> Option<&WorkItem> {
        self.overview().selected_item()
    }

    pub fn stage_counts(&self) -> Vec<StageCount> {
        self.overview().stage_counts()
    }

    pub fn view_scope(&self) -> ViewScope {
        self.overview().view_scope()
    }

    pub fn visible_items(&self) -> &[WorkItem] {
        self.overview().visible_items()
    }

    pub fn archived_items(&self) -> &[WorkItem] {
        self.overview().archived_items()
    }

    pub fn archived_count(&self) -> Option<usize> {
        self.overview().archived_count()
    }

    pub fn archive_error(&self) -> Option<&str> {
        self.overview().archive_error()
    }

    pub fn last_refresh_error(&self) -> Option<&str> {
        match &self.mode {
            AppMode::Overview(session) => session.active_state().last_refresh_error(),
            AppMode::PickerOverlay { underlying, .. } => {
                underlying.active_state().last_refresh_error()
            }
            AppMode::Picker(_) => None,
        }
    }

    pub fn set_refresh_error(&mut self, message: String) {
        match &mut self.mode {
            AppMode::Overview(session) => session.active_state_mut().set_refresh_error(message),
            AppMode::PickerOverlay { underlying, .. } => {
                underlying.active_state_mut().set_refresh_error(message)
            }
            AppMode::Picker(_) => {}
        }
    }

    pub fn reload_from_snapshot(&mut self, snapshot: WorkflowSnapshot) {
        match &mut self.mode {
            AppMode::Overview(session) => session.active_state_mut().reload_from_snapshot(snapshot),
            AppMode::PickerOverlay { underlying, .. } => {
                underlying.active_state_mut().reload_from_snapshot(snapshot)
            }
            AppMode::Picker(_) => {}
        }
    }

    pub fn reload(&mut self) -> Result<(), ParseError> {
        match &mut self.mode {
            AppMode::Overview(session) => session.active_state_mut().reload(),
            AppMode::PickerOverlay { underlying, .. } => underlying.active_state_mut().reload(),
            AppMode::Picker(_) => Ok(()),
        }
    }

    /// Materialize the active slot of the current session by loading from
    /// disk. Used by the event loop after a `WorkflowSwitch` with
    /// `needs_first_load == true`. On parse failure, installs a synthetic
    /// empty `OverviewState` with `last_refresh_error` set so the user sees
    /// the breadcrumb and the error rather than a hang or silent revert.
    pub fn materialize_active(&mut self) {
        let session = match &mut self.mode {
            AppMode::Overview(s) => s,
            _ => return,
        };
        let dir = session.active_dir().to_path_buf();
        match OverviewState::load(dir.clone()) {
            Ok(state) => session.install_active_state(state),
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
            AppMode::Overview(session) => Some(handle_overview_key(session, key)),
            _ => None,
        };
        if let Some(action) = overview_action {
            self.apply_overview_key_action(action);
            return;
        }

        match &mut self.mode {
            AppMode::Overview(_) => {}
            AppMode::Picker(state) => match key.code {
                KeyCode::Char('?') => self.help_open = true,
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Down | KeyCode::Char('j') => state.select_next(),
                KeyCode::Up | KeyCode::Char('k') => state.select_previous(),
                KeyCode::Home => state.select_first(),
                KeyCode::End => state.select_last(),
                KeyCode::Enter => {
                    if let Some(next_mode) = picker_enter_transition(state) {
                        self.mode = next_mode;
                    }
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
                KeyCode::Home => picker.select_first(),
                KeyCode::End => picker.select_last(),
                KeyCode::Enter => {
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
                    // Apply discovery list from the picker into the session,
                    // then select the chosen workflow.
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
                _ => {}
            },
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
                self.pending_switch = Some(workflow_switch);
            }
            OverviewKeyAction::OpenPickerOverlay => {
                self.pending_overlay_open = true;
            }
        }
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
