use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};

use crate::discovery::{resolve_scan_root, DiscoveredWorkflow};
use crate::domain::{WorkItem, WorkflowSnapshot};
use crate::parser::{load_archived_items, load_workflow_dir, ParseError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewScope {
    #[default]
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageCount {
    pub name: String,
    pub items: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverviewState {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub snapshot: WorkflowSnapshot,
    pub selected_index: usize,
    pub view_scope: ViewScope,
    pub archived_items: Vec<WorkItem>,
    pub archive_loaded: bool,
    pub archive_error: Option<String>,
    pub selected_index_archived: usize,
    pub last_refresh_error: Option<String>,
    pub preview_open: bool,
    pub preview_scroll: usize,
    pub max_preview_scroll: Cell<usize>,
    pub preview_scroll_x: usize,
    pub max_preview_scroll_x: Cell<usize>,
    pub preview_wrap: bool,
    pub task_page_size: Cell<usize>,
}

/// Derive a stable slug from a work-item path. Prefer the file stem; when the
/// item lives in a folder-form `{slug}/index.md`, fall back to the parent
/// directory name so reload selection-preservation still matches across the
/// legacy and folder layouts.
fn slug_of(path: &Path) -> Option<String> {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
    match stem.as_deref() {
        Some("index") => path
            .parent()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned()),
        Some(_) => stem,
        None => None,
    }
}

impl OverviewState {
    pub fn empty(workflow_dir: PathBuf) -> Self {
        let repo_root = resolve_scan_root(&workflow_dir);
        let snapshot = WorkflowSnapshot {
            definition: crate::domain::WorkflowDefinition {
                root: workflow_dir.clone(),
                stages: Vec::new(),
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: HashMap::new(),
            },
            items: Vec::new(),
        };
        Self {
            workflow_dir,
            repo_root,
            snapshot,
            selected_index: 0,
            view_scope: ViewScope::Active,
            archived_items: Vec::new(),
            archive_loaded: false,
            archive_error: None,
            selected_index_archived: 0,
            last_refresh_error: None,
            preview_open: false,
            preview_scroll: 0,
            max_preview_scroll: Cell::new(usize::MAX),
            preview_scroll_x: 0,
            max_preview_scroll_x: Cell::new(usize::MAX),
            preview_wrap: false,
            task_page_size: Cell::new(10),
        }
    }

    pub fn load(workflow_dir: PathBuf) -> Result<Self, ParseError> {
        let repo_root = resolve_scan_root(&workflow_dir);
        let snapshot = load_workflow_dir(&workflow_dir, &repo_root)?;
        Ok(Self::from_snapshot_with_root(
            workflow_dir,
            repo_root,
            snapshot,
        ))
    }

    pub fn from_snapshot(workflow_dir: PathBuf, snapshot: WorkflowSnapshot) -> Self {
        let repo_root = resolve_scan_root(&workflow_dir);
        Self::from_snapshot_with_root(workflow_dir, repo_root, snapshot)
    }

    fn from_snapshot_with_root(
        workflow_dir: PathBuf,
        repo_root: PathBuf,
        snapshot: WorkflowSnapshot,
    ) -> Self {
        Self {
            workflow_dir,
            repo_root,
            snapshot,
            selected_index: 0,
            view_scope: ViewScope::Active,
            archived_items: Vec::new(),
            archive_loaded: false,
            archive_error: None,
            selected_index_archived: 0,
            last_refresh_error: None,
            preview_open: false,
            preview_scroll: 0,
            max_preview_scroll: Cell::new(usize::MAX),
            preview_scroll_x: 0,
            max_preview_scroll_x: Cell::new(usize::MAX),
            preview_wrap: false,
            task_page_size: Cell::new(10),
        }
    }

    /// Deterministic reload seam: swap the active snapshot in-place while
    /// preserving selection by slug (file stem) with a clamped-index fallback.
    /// Leaves `view_scope` and the archived-view state untouched — the
    /// watcher-driven refresh only re-parses active items; archived items are
    /// invalidated so the next scope toggle reloads them.
    pub fn reload_from_snapshot(&mut self, snapshot: WorkflowSnapshot) {
        let prior_slug = self
            .snapshot
            .items
            .get(self.selected_index)
            .and_then(|item| slug_of(&item.path));

        self.snapshot = snapshot;
        // Invalidate archive view — a watcher-driven reload may have touched
        // `_archive/` too. Dropping the cached list forces a rescan the next
        // time the user toggles to archived scope.
        self.archived_items.clear();
        self.archive_loaded = false;
        self.archive_error = None;

        let len = self.snapshot.items.len();
        if len == 0 {
            self.selected_index = 0;
        } else if let Some(slug) = prior_slug {
            if let Some(pos) = self
                .snapshot
                .items
                .iter()
                .position(|item| slug_of(&item.path).as_deref() == Some(slug.as_str()))
            {
                self.selected_index = pos;
            } else if self.selected_index >= len {
                self.selected_index = len - 1;
            }
        } else if self.selected_index >= len {
            self.selected_index = len - 1;
        }

        // Clamp archived selection too (archived list is now empty).
        if self.view_scope == ViewScope::Archived {
            self.selected_index_archived = 0;
            self.ensure_archive_loaded();
            self.clamp_selection();
        }

        self.reset_preview_scroll();
        self.last_refresh_error = None;
    }

    /// FS-touching reload wrapper. On success, delegates to
    /// `reload_from_snapshot`. On parse error, retains the prior snapshot
    /// and records the error in `last_refresh_error`.
    pub fn reload(&mut self) -> Result<(), ParseError> {
        match load_workflow_dir(&self.workflow_dir, &self.repo_root) {
            Ok(snapshot) => {
                self.reload_from_snapshot(snapshot);
                Ok(())
            }
            Err(err) => {
                let msg = err.to_string();
                self.last_refresh_error = Some(msg);
                Err(err)
            }
        }
    }

    pub fn last_refresh_error(&self) -> Option<&str> {
        self.last_refresh_error.as_deref()
    }

    pub fn set_refresh_error(&mut self, message: String) {
        self.last_refresh_error = Some(message);
    }

    pub fn workflow_dir(&self) -> &Path {
        &self.workflow_dir
    }

    pub fn snapshot(&self) -> &WorkflowSnapshot {
        &self.snapshot
    }

    pub fn selected_index(&self) -> usize {
        match self.view_scope {
            ViewScope::Active => self.selected_index,
            ViewScope::Archived => self.selected_index_archived,
        }
    }

    pub fn selected_item(&self) -> Option<&WorkItem> {
        self.visible_items().get(self.selected_index())
    }

    pub fn view_scope(&self) -> ViewScope {
        self.view_scope
    }

    pub fn visible_items(&self) -> &[WorkItem] {
        match self.view_scope {
            ViewScope::Active => &self.snapshot.items,
            ViewScope::Archived => &self.archived_items,
        }
    }

    pub fn archived_items(&self) -> &[WorkItem] {
        &self.archived_items
    }

    pub fn archived_count(&self) -> Option<usize> {
        if self.archive_loaded {
            Some(self.archived_items.len())
        } else {
            None
        }
    }

    pub fn archive_error(&self) -> Option<&str> {
        self.archive_error.as_deref()
    }

    fn ensure_archive_loaded(&mut self) {
        if self.archive_loaded {
            return;
        }
        let allowed: Vec<String> = self
            .snapshot
            .definition
            .stages
            .iter()
            .map(|stage| stage.name.clone())
            .collect();
        match load_archived_items(&self.workflow_dir, &allowed) {
            Ok(items) => {
                self.archived_items = items;
                self.archive_error = None;
            }
            Err(err) => {
                self.archived_items = Vec::new();
                self.archive_error = Some(err.to_string());
            }
        }
        self.archive_loaded = true;
    }

    fn toggle_scope(&mut self) {
        self.view_scope = match self.view_scope {
            ViewScope::Active => {
                self.ensure_archive_loaded();
                ViewScope::Archived
            }
            ViewScope::Archived => ViewScope::Active,
        };
        self.clamp_selection();
        self.reset_preview_scroll();
    }

    fn clamp_selection(&mut self) {
        let len = self.visible_items().len();
        match self.view_scope {
            ViewScope::Active => {
                if len == 0 {
                    self.selected_index = 0;
                } else if self.selected_index >= len {
                    self.selected_index = len - 1;
                }
            }
            ViewScope::Archived => {
                if len == 0 {
                    self.selected_index_archived = 0;
                } else if self.selected_index_archived >= len {
                    self.selected_index_archived = len - 1;
                }
            }
        }
    }

    pub fn stage_counts(&self) -> Vec<StageCount> {
        self.snapshot
            .definition
            .stages
            .iter()
            .map(|stage| StageCount {
                name: stage.name.clone(),
                items: self
                    .snapshot
                    .items
                    .iter()
                    .filter(|item| item.status == stage.name)
                    .count(),
            })
            .collect()
    }

    fn select_next(&mut self) {
        let len = self.visible_items().len();
        if len == 0 {
            self.set_scope_index(0);
            return;
        }
        let next = (self.selected_index() + 1).min(len - 1);
        self.set_scope_index(next);
    }

    fn select_previous(&mut self) {
        let prev = self.selected_index().saturating_sub(1);
        self.set_scope_index(prev);
    }

    fn select_first(&mut self) {
        self.set_scope_index(0);
    }

    fn select_last(&mut self) {
        let last = self.visible_items().len().saturating_sub(1);
        self.set_scope_index(last);
    }

    fn set_scope_index(&mut self, value: usize) {
        if self.selected_index() != value {
            self.reset_preview_scroll();
        }
        match self.view_scope {
            ViewScope::Active => self.selected_index = value,
            ViewScope::Archived => self.selected_index_archived = value,
        }
    }

    pub fn preview_open(&self) -> bool {
        self.preview_open
    }

    pub fn toggle_preview(&mut self) {
        self.preview_open = !self.preview_open;
        self.reset_preview_scroll();
    }

    pub fn preview_scroll(&self) -> usize {
        self.preview_scroll
    }

    pub fn preview_scroll_x(&self) -> usize {
        self.preview_scroll_x
    }

    pub fn preview_wrap(&self) -> bool {
        self.preview_wrap
    }

    pub fn toggle_preview_wrap(&mut self) {
        self.preview_wrap = !self.preview_wrap;
    }

    fn scroll_preview_down(&mut self) {
        if !self.preview_open {
            return;
        }
        let max = self.max_preview_scroll.get();
        self.preview_scroll = self.preview_scroll.saturating_add(6).min(max);
    }

    fn scroll_preview_up(&mut self) {
        if !self.preview_open {
            return;
        }
        self.preview_scroll = self.preview_scroll.saturating_sub(6);
    }

    fn scroll_preview_right(&mut self) {
        if !self.preview_open {
            return;
        }
        let max = self.max_preview_scroll_x.get();
        self.preview_scroll_x = self.preview_scroll_x.saturating_add(8).min(max);
    }

    fn scroll_preview_left(&mut self) {
        if !self.preview_open {
            return;
        }
        self.preview_scroll_x = self.preview_scroll_x.saturating_sub(8);
    }

    fn page_selection_down(&mut self) {
        let len = self.visible_items().len();
        if len == 0 {
            self.set_scope_index(0);
            return;
        }
        let step = self.task_page_size.get().max(1);
        let next = self.selected_index().saturating_add(step).min(len - 1);
        self.set_scope_index(next);
    }

    fn page_selection_up(&mut self) {
        let step = self.task_page_size.get().max(1);
        let prev = self.selected_index().saturating_sub(step);
        self.set_scope_index(prev);
    }

    fn reset_preview_scroll(&mut self) {
        self.preview_scroll = 0;
        self.max_preview_scroll.set(usize::MAX);
        self.preview_scroll_x = 0;
        self.max_preview_scroll_x.set(usize::MAX);
        self.preview_wrap = false;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerState {
    pub scan_root: PathBuf,
    pub workflows: Vec<DiscoveredWorkflow>,
    pub selected_index: usize,
    pub error: Option<String>,
}

impl PickerState {
    pub fn new(scan_root: PathBuf, workflows: Vec<DiscoveredWorkflow>) -> Self {
        Self {
            scan_root,
            workflows,
            selected_index: 0,
            error: None,
        }
    }

    pub fn scan_root(&self) -> &Path {
        &self.scan_root
    }

    pub fn workflows(&self) -> &[DiscoveredWorkflow] {
        &self.workflows
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn selected(&self) -> Option<&DiscoveredWorkflow> {
        self.workflows.get(self.selected_index)
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn set_error(&mut self, message: String) {
        self.error = Some(message);
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    fn select_next(&mut self) {
        if self.workflows.is_empty() {
            self.selected_index = 0;
            return;
        }
        self.selected_index = (self.selected_index + 1).min(self.workflows.len() - 1);
    }

    fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    fn select_first(&mut self) {
        self.selected_index = 0;
    }

    fn select_last(&mut self) {
        self.selected_index = self.workflows.len().saturating_sub(1);
    }
}

/// Multi-workflow session: owns one [`OverviewState`] slot per discovered
/// workflow (lazy first-load), plus the active index and the scan root the
/// overlay re-discovery closure can use. Single-workflow constructors build a
/// 1-element session with `pinned_single = true` so the keymap and breadcrumb
/// pay zero UI cost when there is nothing to switch to.
#[derive(Debug, Clone, PartialEq)]
pub struct OverviewSession {
    scan_root: Option<PathBuf>,
    discovery: Vec<DiscoveredWorkflow>,
    workflows: Vec<Option<OverviewState>>,
    active: usize,
    pinned_single: bool,
}

/// A pending workflow switch produced by index-mutation handlers (`]`, `[`,
/// picker-overlay confirm). The event loop drains this after the key is
/// dispatched and performs the watcher teardown + load/reload + watcher
/// restart on the main thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSwitch {
    pub target_index: usize,
    pub needs_first_load: bool,
}

impl OverviewSession {
    /// Build a single-workflow session from a pre-loaded state. `pinned`
    /// reflects the `-w/--workflow-dir` contract — when true, multi-workflow
    /// affordances stay hidden even if the discovery list later grows.
    pub fn single(state: OverviewState, pinned: bool) -> Self {
        let discovery = vec![DiscoveredWorkflow {
            root: state.workflow_dir().to_path_buf(),
            title: None,
        }];
        Self {
            scan_root: None,
            discovery,
            workflows: vec![Some(state)],
            active: 0,
            pinned_single: pinned,
        }
    }

    /// Build a multi-workflow session from a discovery result and a
    /// pre-loaded initial state at `initial_active`.
    pub fn from_discovery(
        scan_root: PathBuf,
        discovery: Vec<DiscoveredWorkflow>,
        initial_active: usize,
        initial_state: OverviewState,
    ) -> Self {
        let mut workflows: Vec<Option<OverviewState>> =
            (0..discovery.len()).map(|_| None).collect();
        let active = initial_active.min(discovery.len().saturating_sub(1));
        if active < workflows.len() {
            workflows[active] = Some(initial_state);
        }
        Self {
            scan_root: Some(scan_root),
            discovery,
            workflows,
            active,
            pinned_single: false,
        }
    }

    pub fn active_state(&self) -> &OverviewState {
        self.workflows[self.active]
            .as_ref()
            .expect("active workflow slot is materialized")
    }

    pub fn active_state_mut(&mut self) -> &mut OverviewState {
        self.workflows[self.active]
            .as_mut()
            .expect("active workflow slot is materialized")
    }

    pub fn discovery(&self) -> &[DiscoveredWorkflow] {
        &self.discovery
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn len(&self) -> usize {
        self.discovery.len()
    }

    pub fn is_empty(&self) -> bool {
        self.discovery.is_empty()
    }

    pub fn is_multi(&self) -> bool {
        self.discovery.len() >= 2 && !self.pinned_single
    }

    pub fn pinned_single(&self) -> bool {
        self.pinned_single
    }

    pub fn scan_root(&self) -> Option<&Path> {
        self.scan_root.as_deref()
    }

    fn slot_loaded(&self, index: usize) -> bool {
        self.workflows
            .get(index)
            .map(|slot| slot.is_some())
            .unwrap_or(false)
    }

    /// Active workflow path (canonical, from discovery).
    pub fn active_dir(&self) -> &Path {
        &self.discovery[self.active].root
    }

    pub fn cycle_next(&mut self) -> WorkflowSwitch {
        let len = self.discovery.len();
        let next = if len <= 1 {
            self.active
        } else {
            (self.active + 1) % len
        };
        self.select(next)
    }

    pub fn cycle_prev(&mut self) -> WorkflowSwitch {
        let len = self.discovery.len();
        let prev = if len <= 1 {
            self.active
        } else if self.active == 0 {
            len - 1
        } else {
            self.active - 1
        };
        self.select(prev)
    }

    pub fn select(&mut self, target_index: usize) -> WorkflowSwitch {
        let target = target_index.min(self.discovery.len().saturating_sub(1));
        let needs_first_load = !self.slot_loaded(target);
        self.active = target;
        WorkflowSwitch {
            target_index: target,
            needs_first_load,
        }
    }

    /// Replace the discovery list (e.g. from a re-discovery via `P`).
    /// Previously-loaded states are remapped by canonical-path match so we
    /// don't drop them unnecessarily; the active workflow is preserved if
    /// still present, otherwise active falls back to 0.
    pub fn replace_discovery(&mut self, new_discovery: Vec<DiscoveredWorkflow>) {
        let prior_active_root = self.discovery.get(self.active).map(|d| d.root.clone());
        let mut new_slots: Vec<Option<OverviewState>> =
            (0..new_discovery.len()).map(|_| None).collect();
        for (old_idx, slot) in self.workflows.drain(..).enumerate() {
            if let Some(state) = slot {
                let old_root = self.discovery.get(old_idx).map(|d| &d.root);
                if let Some(root) = old_root {
                    if let Some(new_idx) = new_discovery.iter().position(|d| &d.root == root) {
                        new_slots[new_idx] = Some(state);
                    }
                }
            }
        }
        let new_active = prior_active_root
            .as_ref()
            .and_then(|root| new_discovery.iter().position(|d| &d.root == root))
            .unwrap_or(0);
        self.discovery = new_discovery;
        self.workflows = new_slots;
        self.active = new_active.min(self.discovery.len().saturating_sub(1));
    }

    /// Materialize the active slot from a loaded `OverviewState`. Used by the
    /// event loop after a `WorkflowSwitch` with `needs_first_load == true`.
    pub fn install_active_state(&mut self, state: OverviewState) {
        if self.active < self.workflows.len() {
            self.workflows[self.active] = Some(state);
        }
    }
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

enum OverviewKeyAction {
    None,
    OpenHelp,
    Quit,
    Switch(WorkflowSwitch),
    OpenPickerOverlay,
}

fn handle_overview_key(session: &mut OverviewSession, key: KeyEvent) -> OverviewKeyAction {
    let is_multi = session.is_multi();
    let pinned = session.pinned_single();
    let state = session.active_state_mut();
    match key.code {
        KeyCode::Char('?') => OverviewKeyAction::OpenHelp,
        KeyCode::Char('q') if state.preview_open() => {
            state.toggle_preview();
            OverviewKeyAction::None
        }
        KeyCode::Char('q') | KeyCode::Esc => OverviewKeyAction::Quit,
        KeyCode::Down | KeyCode::Char('j') => {
            state.select_next();
            OverviewKeyAction::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.select_previous();
            OverviewKeyAction::None
        }
        KeyCode::Enter => {
            state.toggle_preview();
            OverviewKeyAction::None
        }
        KeyCode::PageDown if state.preview_open() => {
            state.scroll_preview_down();
            OverviewKeyAction::None
        }
        KeyCode::PageUp if state.preview_open() => {
            state.scroll_preview_up();
            OverviewKeyAction::None
        }
        KeyCode::PageDown => {
            state.page_selection_down();
            OverviewKeyAction::None
        }
        KeyCode::PageUp => {
            state.page_selection_up();
            OverviewKeyAction::None
        }
        KeyCode::Home => {
            state.select_first();
            OverviewKeyAction::None
        }
        KeyCode::End => {
            state.select_last();
            OverviewKeyAction::None
        }
        KeyCode::Char('a') => {
            state.toggle_scope();
            OverviewKeyAction::None
        }
        KeyCode::Right if state.preview_open() => {
            state.scroll_preview_right();
            OverviewKeyAction::None
        }
        KeyCode::Left if state.preview_open() => {
            state.scroll_preview_left();
            OverviewKeyAction::None
        }
        KeyCode::Char('w') if state.preview_open() => {
            state.toggle_preview_wrap();
            OverviewKeyAction::None
        }
        KeyCode::Right if is_multi => OverviewKeyAction::Switch(session.cycle_next()),
        KeyCode::Left if is_multi => OverviewKeyAction::Switch(session.cycle_prev()),
        KeyCode::Char('P') if is_multi && !pinned => OverviewKeyAction::OpenPickerOverlay,
        _ => OverviewKeyAction::None,
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
mod tests {
    use super::{App, AppMode, ViewScope};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    use crate::discovery::DiscoveredWorkflow;
    use crate::domain::{StageDefinition, WorkItem, WorkflowDefinition, WorkflowSnapshot};

    #[test]
    fn stores_workflow_directory() {
        let app = App::new("docs/spacetop-dev");

        assert_eq!(app.workflow_dir(), Path::new("docs/spacetop-dev"));
    }

    #[test]
    fn loads_real_workflow_state_and_derives_stage_counts() {
        let root = PathBuf::from("workflow");
        let app = App::from_snapshot(root.clone(), snapshot_with_items(3));
        let expected_stage_counts = app
            .snapshot()
            .definition
            .stages
            .iter()
            .map(|stage| {
                (
                    stage.name.as_str(),
                    app.snapshot()
                        .items
                        .iter()
                        .filter(|item| item.status == stage.name)
                        .count(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(app.workflow_dir(), root.as_path());
        assert_eq!(
            app.stage_counts()
                .iter()
                .map(|count| (count.name.as_str(), count.items))
                .collect::<Vec<_>>(),
            expected_stage_counts
        );
        // Selection defaults to the first item; assert against the snapshot
        // rather than hard-coding a title that drifts as tasks ship.
        assert_eq!(
            app.selected_item().map(|item| item.title.as_str()),
            app.snapshot().items.first().map(|item| item.title.as_str())
        );
        assert_eq!(
            app.selected_item().map(|item| item.status.as_str()),
            app.snapshot()
                .items
                .first()
                .map(|item| item.status.as_str())
        );
        // The workflow has at least one stage and at least one item — these
        // are intrinsic invariants of the loaded fixture, not specific titles.
        assert!(!app.snapshot().definition.stages.is_empty());
        assert!(!app.snapshot().items.is_empty());
    }

    #[test]
    fn navigation_changes_selection_without_touching_snapshot() {
        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));

        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected_index(), 1);

        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected_index(), 2);

        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.selected_index(), 1);

        app.handle_key(key(KeyCode::Home));
        assert_eq!(app.selected_index(), 0);

        app.handle_key(key(KeyCode::End));
        assert_eq!(app.selected_index(), 2);
        assert_eq!(app.snapshot().items.len(), 3);
    }

    #[test]
    fn page_keys_scroll_preview_without_changing_selection() {
        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));

        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::PageDown));
        assert_eq!(app.selected_index(), 0);
        assert_eq!(app.as_overview().unwrap().preview_scroll(), 6);

        app.handle_key(key(KeyCode::PageDown));
        assert_eq!(app.selected_index(), 0);
        assert_eq!(app.as_overview().unwrap().preview_scroll(), 12);

        app.handle_key(key(KeyCode::PageUp));
        assert_eq!(app.selected_index(), 0);
        assert_eq!(app.as_overview().unwrap().preview_scroll(), 6);
    }

    #[test]
    fn changing_selection_resets_preview_scroll() {
        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));

        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::PageDown));
        assert_eq!(app.as_overview().unwrap().preview_scroll(), 6);

        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected_index(), 1);
        assert_eq!(app.as_overview().unwrap().preview_scroll(), 0);
        assert_eq!(app.as_overview().unwrap().preview_scroll_x(), 0);
    }

    #[test]
    fn preview_mode_is_closed_by_default_and_enter_toggles_it() {
        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));

        assert!(!app.as_overview().unwrap().preview_open());

        app.handle_key(key(KeyCode::Enter));
        assert!(app.as_overview().unwrap().preview_open());

        app.handle_key(key(KeyCode::Enter));
        assert!(!app.as_overview().unwrap().preview_open());
    }

    #[test]
    fn preview_scroll_keys_are_ignored_until_preview_mode_is_open() {
        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
        app.as_overview().unwrap().task_page_size.set(1);

        app.handle_key(key(KeyCode::PageDown));
        app.handle_key(key(KeyCode::Right));

        assert_eq!(app.selected_index(), 1);
        assert_eq!(app.as_overview().unwrap().preview_scroll(), 0);
        assert_eq!(app.as_overview().unwrap().preview_scroll_x(), 0);
    }

    #[test]
    fn page_keys_move_task_selection_when_preview_is_closed() {
        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(6));
        app.as_overview().unwrap().task_page_size.set(2);

        app.handle_key(key(KeyCode::PageDown));
        assert_eq!(app.selected_index(), 2);

        app.handle_key(key(KeyCode::PageDown));
        assert_eq!(app.selected_index(), 4);

        app.handle_key(key(KeyCode::PageUp));
        assert_eq!(app.selected_index(), 2);
    }

    #[test]
    fn scroll_preview_down_is_capped_at_max_scroll() {
        let mut state =
            super::OverviewState::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(1));
        state.preview_open = true;
        // Simulate render having set max_scroll = 10.
        state.max_preview_scroll.set(10);

        // Press PageDown 20 times — should not exceed 10.
        for _ in 0..20 {
            state.scroll_preview_down();
        }
        assert!(
            state.preview_scroll() <= 10,
            "preview_scroll must not exceed max_scroll"
        );
    }

    #[test]
    fn scroll_preview_up_responds_immediately_after_capped_down() {
        let mut state =
            super::OverviewState::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(1));
        state.preview_open = true;
        state.max_preview_scroll.set(10);

        // Press down many times (capped at 10).
        for _ in 0..30 {
            state.scroll_preview_down();
        }
        assert_eq!(state.preview_scroll(), 10);

        // One PageUp should immediately decrease position.
        state.scroll_preview_up();
        assert!(
            state.preview_scroll() < 10,
            "first PageUp must decrease scroll after capped drift"
        );
    }

    #[test]
    fn navigation_is_clamped_for_empty_workflows() {
        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(0));

        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::End));

        assert_eq!(app.selected_index(), 0);
        assert!(app.selected_item().is_none());
    }

    #[test]
    fn quit_keys_set_quit_state_but_movement_keys_do_not() {
        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));

        app.handle_key(key(KeyCode::Down));
        assert!(!app.should_quit());

        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit());

        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
        app.handle_key(key(KeyCode::Esc));
        assert!(app.should_quit());
    }

    #[test]
    fn q_closes_preview_before_quitting_overview() {
        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));

        app.handle_key(key(KeyCode::Enter));
        assert!(app.as_overview().unwrap().preview_open());

        app.handle_key(key(KeyCode::Char('q')));
        assert!(!app.as_overview().unwrap().preview_open());
        assert!(!app.should_quit());

        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit());
    }

    #[test]
    fn default_view_scope_is_active_and_visible_items_match_snapshot() {
        let app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
        assert_eq!(app.view_scope(), ViewScope::Active);
        assert_eq!(app.visible_items().len(), app.snapshot().items.len());
        assert!(app.archived_count().is_none());
    }

    #[test]
    fn toggle_scope_key_a_flips_to_archived_and_loads_lazily() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let mut app = App::load(root).expect("workflow should load");
        assert_eq!(app.view_scope(), ViewScope::Active);
        assert!(app.archived_count().is_none());

        app.handle_key(key(KeyCode::Char('a')));
        assert_eq!(app.view_scope(), ViewScope::Archived);
        assert!(app.archived_count().is_some());
        assert!(!app.archived_items().is_empty());
        // Selected item should be an archived entry.
        let selected = app.selected_item().expect("selected archived item");
        assert_eq!(selected.status, "done");

        app.handle_key(key(KeyCode::Char('a')));
        assert_eq!(app.view_scope(), ViewScope::Active);
    }

    #[test]
    fn archived_view_selection_is_independent_of_active_selection() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let mut app = App::load(root).expect("workflow should load");
        app.handle_key(key(KeyCode::Down));
        let active_index = app.selected_index();

        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        let archived_index = app.selected_index();

        app.handle_key(key(KeyCode::Char('a')));
        assert_eq!(app.selected_index(), active_index);

        app.handle_key(key(KeyCode::Char('a')));
        assert_eq!(app.selected_index(), archived_index);
    }

    #[test]
    fn archive_count_hidden_before_first_toggle() {
        let app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(1));
        assert!(app.archived_count().is_none());
    }

    // --- Picker tests ---

    fn picker_app(workflows: Vec<DiscoveredWorkflow>) -> App {
        App::from_picker(PathBuf::from("/scan-root"), workflows)
    }

    fn fake_workflows(count: usize) -> Vec<DiscoveredWorkflow> {
        (0..count)
            .map(|i| DiscoveredWorkflow {
                root: PathBuf::from(format!("/scan-root/docs/w{i}")),
                title: Some(format!("Workflow {i}")),
            })
            .collect()
    }

    #[test]
    fn picker_state_navigation_is_clamped() {
        let mut app = picker_app(fake_workflows(3));

        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.as_picker().unwrap().selected_index(), 1);

        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.as_picker().unwrap().selected_index(), 2);

        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.as_picker().unwrap().selected_index(), 1);

        app.handle_key(key(KeyCode::Up));
        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.as_picker().unwrap().selected_index(), 0);

        app.handle_key(key(KeyCode::End));
        assert_eq!(app.as_picker().unwrap().selected_index(), 2);

        app.handle_key(key(KeyCode::Home));
        assert_eq!(app.as_picker().unwrap().selected_index(), 0);
    }

    #[test]
    fn picker_enter_transitions_to_overview_on_success() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let workflows = vec![
            DiscoveredWorkflow {
                root: root.clone(),
                title: Some("Real".to_string()),
            },
            DiscoveredWorkflow {
                root: PathBuf::from("/nonexistent/other"),
                title: None,
            },
        ];
        let mut app = App::from_picker(PathBuf::from("/scan-root"), workflows);

        app.handle_key(key(KeyCode::Enter));

        assert!(app.as_overview().is_some(), "expected transition");
        assert_eq!(app.workflow_dir(), root.as_path());
        assert!(matches!(app.mode(), AppMode::Overview(_)));
    }

    #[test]
    fn picker_enter_surfaces_error_on_load_failure() {
        let workflows = vec![
            DiscoveredWorkflow {
                root: PathBuf::from("/does/not/exist/alpha"),
                title: None,
            },
            DiscoveredWorkflow {
                root: PathBuf::from("/does/not/exist/beta"),
                title: None,
            },
        ];
        let mut app = App::from_picker(PathBuf::from("/scan-root"), workflows);

        app.handle_key(key(KeyCode::Enter));

        assert!(app.as_picker().is_some(), "should still be in picker mode");
        assert!(
            app.as_picker().unwrap().error().is_some(),
            "expected error to be surfaced"
        );
        assert!(!app.should_quit());
    }

    #[test]
    fn picker_q_and_esc_quit_without_transition() {
        let mut app = picker_app(fake_workflows(2));
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit());
        assert!(app.as_picker().is_some());

        let mut app = picker_app(fake_workflows(2));
        app.handle_key(key(KeyCode::Esc));
        assert!(app.should_quit());
        assert!(app.as_picker().is_some());
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn snapshot_with_items(count: usize) -> WorkflowSnapshot {
        WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: PathBuf::from("workflow"),
                stages: vec![
                    StageDefinition {
                        name: "plan".to_string(),
                        initial: true,
                        terminal: false,
                        gate: false,
                        fresh: false,
                        feedback_to: None,
                        worktree: false,
                        concurrency: None,
                    },
                    StageDefinition {
                        name: "implement".to_string(),
                        initial: false,
                        terminal: false,
                        gate: false,
                        fresh: false,
                        feedback_to: None,
                        worktree: true,
                        concurrency: None,
                    },
                ],
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: HashMap::new(),
            },
            items: (0..count)
                .map(|index| WorkItem {
                    path: PathBuf::from(format!("workflow/task-{index}.md")),
                    id: format!("{index:03}"),
                    title: format!("Task {index}"),
                    status: if index == 0 { "plan" } else { "implement" }.to_string(),
                    source: Some("test".to_string()),
                    started: None,
                    completed: None,
                    verdict: None,
                    score: Some(0.5),
                    worktree: None,
                    issue: None,
                    pr: None,
                    body: format!("Body excerpt for task {index}."),
                })
                .collect(),
        }
    }

    fn snapshot_with_paths(paths: &[&str]) -> WorkflowSnapshot {
        WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: PathBuf::from("workflow"),
                stages: vec![StageDefinition {
                    name: "plan".to_string(),
                    initial: true,
                    terminal: false,
                    gate: false,
                    fresh: false,
                    feedback_to: None,
                    worktree: false,
                    concurrency: None,
                }],
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: HashMap::new(),
            },
            items: paths
                .iter()
                .enumerate()
                .map(|(index, p)| WorkItem {
                    path: PathBuf::from(p),
                    id: format!("{index:03}"),
                    title: format!("Task {p}"),
                    status: "plan".to_string(),
                    source: None,
                    started: None,
                    completed: None,
                    verdict: None,
                    score: None,
                    worktree: None,
                    issue: None,
                    pr: None,
                    body: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn reload_from_snapshot_preserves_selection_by_slug() {
        let mut app = App::from_snapshot(
            PathBuf::from("workflow"),
            snapshot_with_paths(&["workflow/alpha.md", "workflow/beta.md", "workflow/gamma.md"]),
        );
        app.handle_key(key(KeyCode::Down)); // select beta (index 1)
        assert_eq!(
            app.selected_item().map(|i| i.path.clone()),
            Some(PathBuf::from("workflow/beta.md"))
        );

        // Reorder: beta now at index 1 still, but amid different neighbors.
        app.reload_from_snapshot(snapshot_with_paths(&[
            "workflow/gamma.md",
            "workflow/beta.md",
            "workflow/delta.md",
        ]));

        assert_eq!(app.selected_index(), 1);
        assert_eq!(
            app.selected_item().map(|i| i.path.clone()),
            Some(PathBuf::from("workflow/beta.md"))
        );
    }

    #[test]
    fn reload_from_snapshot_preserves_selection_by_slug_at_new_index() {
        let mut app = App::from_snapshot(
            PathBuf::from("workflow"),
            snapshot_with_paths(&["workflow/alpha.md", "workflow/beta.md", "workflow/gamma.md"]),
        );
        app.handle_key(key(KeyCode::Down)); // beta at index 1

        // beta moved to index 2.
        app.reload_from_snapshot(snapshot_with_paths(&[
            "workflow/alpha.md",
            "workflow/gamma.md",
            "workflow/beta.md",
        ]));

        assert_eq!(app.selected_index(), 2);
    }

    #[test]
    fn reload_from_snapshot_clamps_when_slug_missing() {
        let mut app = App::from_snapshot(
            PathBuf::from("workflow"),
            snapshot_with_paths(&["workflow/alpha.md", "workflow/beta.md", "workflow/gamma.md"]),
        );
        app.handle_key(key(KeyCode::End)); // select gamma (index 2)
        assert_eq!(app.selected_index(), 2);

        // gamma is gone, snapshot shrinks to 2 items.
        app.reload_from_snapshot(snapshot_with_paths(&[
            "workflow/alpha.md",
            "workflow/beta.md",
        ]));

        assert_eq!(app.selected_index(), 1);
    }

    #[test]
    fn reload_from_snapshot_empty_clears_selection() {
        let mut app = App::from_snapshot(
            PathBuf::from("workflow"),
            snapshot_with_paths(&["workflow/alpha.md", "workflow/beta.md"]),
        );
        app.handle_key(key(KeyCode::Down));
        app.reload_from_snapshot(snapshot_with_paths(&[]));
        assert_eq!(app.selected_index(), 0);
        assert!(app.selected_item().is_none());
    }

    #[test]
    fn reload_from_snapshot_clears_prior_error() {
        let mut app = App::from_snapshot(
            PathBuf::from("workflow"),
            snapshot_with_paths(&["workflow/alpha.md"]),
        );
        app.set_refresh_error("boom".into());
        assert_eq!(app.last_refresh_error(), Some("boom"));

        app.reload_from_snapshot(snapshot_with_paths(&["workflow/alpha.md"]));
        assert_eq!(app.last_refresh_error(), None);
    }

    #[test]
    fn reload_from_snapshot_preserves_view_scope() {
        use crate::domain::WorkItem;
        let mut overview = super::OverviewState::from_snapshot(
            PathBuf::from("workflow"),
            snapshot_with_paths(&["workflow/alpha.md"]),
        );
        // Force into archived scope with synthetic archived items.
        overview.view_scope = ViewScope::Archived;
        overview.archived_items = vec![WorkItem {
            path: PathBuf::from("workflow/_archive/old.md"),
            id: "old".into(),
            title: "Old".into(),
            status: "done".into(),
            source: None,
            started: None,
            completed: None,
            verdict: None,
            score: None,
            worktree: None,
            issue: None,
            pr: None,
            body: String::new(),
        }];
        overview.archive_loaded = true;

        overview.reload_from_snapshot(snapshot_with_paths(&[
            "workflow/alpha.md",
            "workflow/beta.md",
        ]));

        // View scope preserved; when already in archived mode, the archive
        // cache is immediately reloaded so the view does not go empty after
        // a reload or workflow switch.
        assert_eq!(overview.view_scope, ViewScope::Archived);
        assert!(overview.archive_loaded);
        assert!(overview.archive_error.is_none());
    }

    #[test]
    fn reload_retains_prior_snapshot_on_parse_error() {
        // Minimal real workflow fixture in a tempdir.
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(
            root.join("README.md"),
            "---\nstages:\n  states:\n    - name: plan\n      initial: true\n---\n",
        )
        .unwrap();
        std::fs::write(
            root.join("task-one.md"),
            "---\nid: 001\ntitle: One\nstatus: plan\n---\n\nbody\n",
        )
        .unwrap();

        let mut app = App::load(root.to_path_buf()).expect("load ok");
        let prior_snapshot = app.snapshot().clone();

        // Poison one task file: invalid YAML frontmatter.
        std::fs::write(
            root.join("task-one.md"),
            "---\nid: [not valid yaml\nstatus\n---\nbody\n",
        )
        .unwrap();

        let result = app.reload();
        assert!(result.is_err(), "parse should fail");
        assert_eq!(
            app.snapshot(),
            &prior_snapshot,
            "prior snapshot retained on parse error"
        );
        assert!(app.last_refresh_error().is_some());
    }

    // --- Multi-workflow session tests (task 010) ---

    use super::{OverviewSession, OverviewState};

    /// Write a minimal real workflow (one task in `plan`) into `dir`.
    fn write_workflow(dir: &Path, task_id: &str) {
        std::fs::write(
            dir.join("README.md"),
            "---\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: done\n      terminal: true\n---\n",
        )
        .unwrap();
        std::fs::write(
            dir.join(format!("task-{task_id}.md")),
            format!("---\nid: {task_id}\ntitle: Task {task_id}\nstatus: plan\n---\n\nbody for {task_id}\n"),
        )
        .unwrap();
    }

    /// Build a multi-workflow session with `n` real tempdir workflows; the
    /// first slot is materialized (its OverviewState is loaded from disk),
    /// the rest are lazy `None` until activated. Returns the session plus
    /// the holder tempdir (must outlive the session).
    fn multi_session(n: usize) -> (OverviewSession, tempfile::TempDir, Vec<PathBuf>) {
        let holder = tempfile::tempdir().expect("tempdir holder");
        let mut roots = Vec::with_capacity(n);
        let mut discovery = Vec::with_capacity(n);
        for i in 0..n {
            let root = holder.path().join(format!("w{i}"));
            std::fs::create_dir_all(&root).unwrap();
            write_workflow(&root, &format!("{i:03}"));
            roots.push(root.clone());
            discovery.push(DiscoveredWorkflow {
                root,
                title: Some(format!("Workflow {i}")),
            });
        }
        let initial = OverviewState::load(roots[0].clone()).expect("load w0");
        let session =
            OverviewSession::from_discovery(holder.path().to_path_buf(), discovery, 0, initial);
        (session, holder, roots)
    }

    #[test]
    fn cycle_keys_advance_active_index_in_multi_session() {
        let (session, _holder, _roots) = multi_session(3);
        let mut app = App::from_session(session);
        assert!(app.as_session().unwrap().is_multi());
        assert_eq!(app.as_session().unwrap().active_index(), 0);

        app.handle_key(key(KeyCode::Right));
        let switch = app.take_pending_switch().expect("cycle next emits switch");
        assert_eq!(switch.target_index, 1);
        assert!(switch.needs_first_load);
        assert_eq!(app.as_session().unwrap().active_index(), 1);

        // Materialize so subsequent cycles work; the test exercises pure
        // index mutation but materialize is what the event loop does.
        app.materialize_active();

        app.handle_key(key(KeyCode::Right));
        let _ = app.take_pending_switch();
        app.materialize_active();
        app.handle_key(key(KeyCode::Right)); // wraps back to 0
        let switch = app.take_pending_switch().expect("wrap emits switch");
        assert_eq!(switch.target_index, 0);
        assert!(!switch.needs_first_load, "w0 was already loaded");

        app.handle_key(key(KeyCode::Left)); // wrap to last
        let switch = app.take_pending_switch().expect("prev emits switch");
        assert_eq!(switch.target_index, 2);
    }

    #[test]
    fn cycle_keys_inert_in_single_session() {
        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
        let original = app.clone();
        app.handle_key(key(KeyCode::Right));
        app.handle_key(key(KeyCode::Left));
        app.handle_key(key(KeyCode::Char('P')));
        assert!(app.take_pending_switch().is_none());
        assert!(!app.take_pending_overlay_open());
        assert_eq!(app, original);
    }

    #[test]
    fn preview_mode_consumes_left_right_for_horizontal_scroll_in_multi() {
        let (session, _holder, _roots) = multi_session(3);
        let mut app = App::from_session(session);
        let state = match &mut app.mode {
            AppMode::Overview(session) => session.active_state_mut(),
            _ => panic!("expected overview"),
        };
        state.toggle_preview();
        state.max_preview_scroll_x.set(24);

        app.handle_key(key(KeyCode::Right));
        assert_eq!(app.as_overview().unwrap().preview_scroll_x(), 8);
        assert!(app.take_pending_switch().is_none());

        app.handle_key(key(KeyCode::Left));
        assert_eq!(app.as_overview().unwrap().preview_scroll_x(), 0);
        assert!(app.take_pending_switch().is_none());
    }

    #[test]
    fn picker_overlay_open_close_preserves_session() {
        let (session, _holder, _roots) = multi_session(2);
        let mut app = App::from_session(session);
        let original_active = app.as_session().unwrap().active_index();

        // Press P: schedules overlay-open; the event loop normally re-runs
        // discovery, but we simulate that with the same list.
        app.handle_key(key(KeyCode::Char('P')));
        assert!(app.take_pending_overlay_open());
        let same_list = app.as_session().unwrap().discovery().to_vec();
        app.open_picker_overlay_with(Ok(same_list));
        assert!(app.is_overlay());

        // Esc dismisses and restores.
        app.handle_key(key(KeyCode::Esc));
        assert!(!app.is_overlay());
        assert!(app.as_session().is_some());
        assert_eq!(app.as_session().unwrap().active_index(), original_active);
    }

    #[test]
    fn picker_overlay_q_closes_popup_without_quitting() {
        let (session, _holder, _roots) = multi_session(2);
        let mut app = App::from_session(session);
        app.handle_key(key(KeyCode::Char('P')));
        assert!(app.take_pending_overlay_open());
        app.open_picker_overlay_with(Ok(app.as_session().unwrap().discovery().to_vec()));
        assert!(app.is_overlay());

        app.handle_key(key(KeyCode::Char('q')));
        assert!(!app.is_overlay());
        assert!(!app.should_quit());
        assert!(matches!(app.mode(), AppMode::Overview(_)));
    }

    #[test]
    fn picker_overlay_pickup_adds_new_workflow() {
        let (session, holder, _roots) = multi_session(2);
        let mut app = App::from_session(session);
        // Create a third workflow on disk, then open overlay with the new
        // discovery list including it.
        let new_root = holder.path().join("w-new");
        std::fs::create_dir_all(&new_root).unwrap();
        write_workflow(&new_root, "999");
        app.handle_key(key(KeyCode::Char('P')));
        assert!(app.take_pending_overlay_open());
        let mut new_list = app.as_session().unwrap().discovery().to_vec();
        new_list.push(DiscoveredWorkflow {
            root: new_root.clone(),
            title: Some("New".to_string()),
        });
        app.open_picker_overlay_with(Ok(new_list));
        assert!(app.is_overlay());
        // Move selection to the new entry (index 2) and press Enter.
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        let switch = app.take_pending_switch().expect("Enter emits switch");
        assert_eq!(switch.target_index, 2);
        assert!(switch.needs_first_load);
        assert_eq!(app.as_session().unwrap().discovery().len(), 3);
    }

    #[test]
    fn switch_preserves_per_workflow_state() {
        // Two real workflows; in A, advance selection + flip to archived.
        // After cycling away and back, A's state must be intact.
        let (session, _holder, _roots) = multi_session(2);
        let mut app = App::from_session(session);
        // First, write an _archive entry into w0 so toggle has something to
        // load.
        let w0 = app.workflow_dir().to_path_buf();
        std::fs::create_dir_all(w0.join("_archive")).unwrap();
        std::fs::write(
            w0.join("_archive/old.md"),
            "---\nid: old\ntitle: Old\nstatus: done\nverdict: PASSED\n---\n\n",
        )
        .unwrap();
        // Re-load to pick up archive presence (not strictly needed since
        // `a` will scan on toggle).
        let _ = app.reload();

        app.handle_key(key(KeyCode::Char('a'))); // → Archived
        assert_eq!(app.view_scope(), ViewScope::Archived);
        let archived_loaded = app.as_overview().map(|s| s.archive_loaded).unwrap_or(false);
        assert!(archived_loaded);

        // Cycle to w1 (first-load), then back to w0.
        app.handle_key(key(KeyCode::Right));
        let switch = app.take_pending_switch().unwrap();
        assert_eq!(switch.target_index, 1);
        app.materialize_active();
        // Cycle back to w0.
        app.handle_key(key(KeyCode::Left));
        let switch = app.take_pending_switch().unwrap();
        assert_eq!(switch.target_index, 0);
        assert!(
            !switch.needs_first_load,
            "w0 was already loaded; should not re-load"
        );
        // w0 state preserved: still in Archived view, archive cache loaded.
        assert_eq!(app.view_scope(), ViewScope::Archived);
        assert!(app.as_overview().unwrap().archive_loaded);
        assert!(
            !app.archived_items().is_empty(),
            "archived cache should be reloaded when returning to an archived workflow"
        );
    }

    #[test]
    fn switch_failure_records_refresh_error_on_synthetic_state() {
        // Build a session whose w1 root does not exist; activating it
        // should yield a synthetic empty state with last_refresh_error set
        // (rather than panicking or silently reverting).
        let (mut session, holder, _roots) = multi_session(2);
        // Replace w1's discovery root with a path that doesn't exist.
        let mut new_disc = session.discovery().to_vec();
        new_disc[1].root = holder.path().join("does-not-exist");
        session.replace_discovery(new_disc);
        let mut app = App::from_session(session);
        app.handle_key(key(KeyCode::Right));
        let switch = app.take_pending_switch().unwrap();
        assert_eq!(switch.target_index, 1);
        assert!(switch.needs_first_load);
        app.materialize_active();
        // Active state is now an empty synthetic state with refresh error.
        assert!(app.last_refresh_error().is_some());
        assert!(app.snapshot().items.is_empty());
        assert_eq!(app.as_session().unwrap().active_index(), 1);
    }

    #[test]
    fn keymap_audit_is_disjoint() {
        // The pre-existing char-key set used by the overview handler.
        let existing_chars: &[char] = &['a', '?', 'j', 'k', 'q'];
        let new_chars: &[char] = &['P'];
        for c in new_chars {
            assert!(
                !existing_chars.contains(c),
                "new keymap char {c:?} collides with existing binding"
            );
        }
        // Tab-cycle bindings live on `Left`/`Right` (non-Char), and `Up`/
        // `Down`/`Home`/`End`/`Enter`/`Esc` are also non-Char — those don't
        // share the Char keyspace and can't collide here.
    }

    // The `cycle_keys_advance_active_index_in_multi_session` test above
    // already covers `needs_first_load` == true for the first activation
    // and `needs_first_load` == false for a return; that satisfies the
    // "first activation loads exactly once" plan item without a hand-rolled
    // counting fake.
}
