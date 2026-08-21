use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ratatui::layout::Rect;

use spacetop_core::config::{DefaultScope, DefaultSort, SpacetopConfig};
use spacetop_core::discovery::resolve_scan_root;
use spacetop_core::domain::{
    Entity, EntityParseError, StateCheckoutDisposition, WorkflowSnapshot, WorkflowStorage,
};
use spacetop_core::entity_identity::entity_slug;
pub use spacetop_core::index::StageCount;
use spacetop_core::index::WorkflowIndex;
use spacetop_core::parser::ParseError;
use spacetop_core::query::{EntityQuery, EntitySort, QueryScope};
use spacetop_core::session_state::{WorkflowScope, WorkflowSession};
use spacetop_core::sources::{ArchiveSnapshot, WorkflowSources};

use super::history_worker::{HistoryWorkerRequest, HistoryWorkerResult};
use super::session_activity_worker::{SessionActivityWorkerRequest, SessionActivityWorkerResult};

/// Selection target in the task list — either a real work item or a synthetic
/// "broken" row representing an entity whose frontmatter failed to parse.
#[derive(Debug, Clone)]
pub enum SelectedRow {
    Item(Box<Entity>),
    Broken(EntityParseError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewScope {
    #[default]
    Active,
    Archived,
}

/// Where the preview pane sits relative to the entity list. Owned by the
/// app layer (not ui) so mouse hit-testing and the per-placement split
/// ratio can reason about it without a terminal backend; `ui::layout`
/// decides which placement applies from the area's aspect ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewPlacement {
    Left,
    Bottom,
}

/// Bounds for the user-draggable list/preview split ratio (percent of the
/// content area given to the list pane). Keeps both panes usable; the
/// geometric minimums in `ui::layout::split_content` clamp further on
/// small terminals.
pub(crate) const SPLIT_PERCENT_MIN: u16 = 10;
pub(crate) const SPLIT_PERCENT_MAX: u16 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    #[default]
    Id,
    Status,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Id => "id",
            SortMode::Status => "status",
        }
    }
}

/// User-visible status of the most recent sync attempt against the
/// active workflow's repo root. Owned per-`OverviewState` so each tab
/// in a multi-workflow session has its own status pill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    /// A `git pull` is in flight on the main thread; the UI shows
    /// `"Syncing…"` and the next event-loop tick replaces it with the
    /// outcome.
    InFlight,
    /// The pull succeeded; either fast-forwarded `new_commits` commits
    /// or was already up to date.
    Succeeded { new_commits: u32 },
    /// Both the definition repository and a verified attached split-root
    /// state checkout completed their fast-forward-only refreshes.
    SucceededWithState { new_commits: u32 },
    /// The definition repository was refreshed, but split-root state was not
    /// verified or could not be refreshed. The readable snapshot is retained.
    Partial { message: String },
    /// The pull was attempted but failed; `message` is the trimmed
    /// stderr / synthesized reason from the helper.
    Failed { message: String },
    /// The repo state precluded a pull (no git repo, no upstream, no
    /// `origin` remote). `hint` is the user-facing description.
    Unavailable { hint: String },
}

/// Stable app-layer diagnostic consumed directly by the UI. This keeps Git
/// and README interpretation out of rendering code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateTopologyDiagnostic {
    Detached,
    WrongBranch {
        actual_branch: String,
        expected_branch: String,
    },
    Missing,
    ProbeFailed {
        reason: String,
    },
}

impl StateTopologyDiagnostic {
    pub fn label(&self) -> String {
        match self {
            Self::Detached => "State detached; snapshot may be stale".to_string(),
            Self::WrongBranch {
                actual_branch,
                expected_branch,
            } => format!("State on branch {actual_branch}; expected {expected_branch}"),
            Self::Missing => "State checkout missing; no state loaded".to_string(),
            Self::ProbeFailed { reason } => format!("State topology unverified: {reason}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverviewState {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub index: WorkflowIndex,
    pub selected_index: usize,
    pub view_scope: ViewScope,
    pub archived_done_count: Option<usize>,
    pub archive_loaded: bool,
    pub archive_error: Option<String>,
    pub selected_index_archived: usize,
    pub last_refresh_error: Option<String>,
    pub preview_open: bool,
    pub preview_scroll: usize,
    pub max_preview_scroll: Cell<usize>,
    pub preview_scroll_x: usize,
    pub max_preview_scroll_x: Cell<usize>,
    /// Visible height (rows) of the preview body area, written by the render
    /// pass each frame (interior mutability, like `max_preview_scroll`). Drives
    /// the page-relative vertical scroll step. The event loop draws before it
    /// polls input (`lib.rs`), so this is always populated before any scroll
    /// key is read.
    pub preview_viewport_height: Cell<usize>,
    pub preview_wrap: bool,
    pub task_page_size: Cell<usize>,
    pub sort_mode: SortMode,
    pub sync_status: Option<SyncStatus>,
    /// Percent of the content area given to the list pane in Left
    /// placement (preview to the right). One ratio per placement so the
    /// session-held drag result survives aspect-ratio flips; defaults
    /// preserve the historical fixed 50/50 split.
    pub split_percent_left: u16,
    /// List-pane percent in Bottom placement (preview below). Default
    /// preserves the historical fixed 30/70 split.
    pub split_percent_bottom: u16,
    /// True while a left-button divider drag is in progress (mouse down on
    /// the divider band, before the matching button-up).
    pub divider_drag: bool,
    /// Render-fact: the content area (list + preview) drawn last frame.
    /// Written by the render pass (interior mutability, like
    /// `max_preview_scroll`); read by mouse hit-testing. The event loop
    /// draws before it polls input, so these are populated before any
    /// mouse event is read.
    pub content_rect: Cell<Rect>,
    /// Render-fact: the list rows area (after the 1-row section header).
    pub list_rows_rect: Cell<Rect>,
    /// Render-fact: the entity-ID cells drawn in the list rows. Its width is
    /// the responsive ID-column width and its height covers only real entity
    /// rows, never synthetic broken rows.
    pub id_column_rect: Cell<Rect>,
    /// Render-fact: first visible list index, from `ListState::offset()`
    /// after the stateful render.
    pub list_offset: Cell<usize>,
    /// Render-fact: the preview pane area (including its divider border).
    /// Reset to `Rect::default()` when the preview is closed so wheel
    /// events never hit a stale rect.
    pub preview_rect: Cell<Rect>,
}

impl OverviewState {
    pub fn empty(workflow_dir: PathBuf) -> Self {
        let repo_root = resolve_scan_root(&workflow_dir);
        let snapshot = WorkflowSnapshot {
            definition: spacetop_core::domain::WorkflowDefinition {
                root: workflow_dir.clone(),
                state: None,
                storage: Default::default(),
                stages: Vec::new(),
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: HashMap::new(),
                stage_prose: HashMap::new(),
                transitions: Vec::new(),
            },
            items: Vec::new(),
            parse_errors: Vec::new(),
        };
        let index = WorkflowIndex::from_sources(WorkflowSources {
            active: snapshot,
            archive: ArchiveSnapshot::empty(),
        });
        Self {
            workflow_dir,
            repo_root,
            index,
            selected_index: 0,
            view_scope: ViewScope::Active,
            archived_done_count: None,
            archive_loaded: false,
            archive_error: None,
            selected_index_archived: 0,
            last_refresh_error: None,
            preview_open: false,
            preview_scroll: 0,
            max_preview_scroll: Cell::new(usize::MAX),
            preview_scroll_x: 0,
            max_preview_scroll_x: Cell::new(usize::MAX),
            preview_viewport_height: Cell::new(0),
            preview_wrap: true,
            task_page_size: Cell::new(10),
            sort_mode: SortMode::default(),
            sync_status: None,
            split_percent_left: 50,
            split_percent_bottom: 30,
            divider_drag: false,
            content_rect: Cell::new(Rect::default()),
            list_rows_rect: Cell::new(Rect::default()),
            id_column_rect: Cell::new(Rect::default()),
            list_offset: Cell::new(0),
            preview_rect: Cell::new(Rect::default()),
        }
    }

    pub fn load(workflow_dir: PathBuf) -> Result<Self, ParseError> {
        let repo_root = resolve_scan_root(&workflow_dir);
        let mut index = WorkflowIndex::load(&workflow_dir, &repo_root)?;
        index.mark_history_loading();
        Ok(Self::from_index_with_root(workflow_dir, repo_root, index))
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
        let index = WorkflowIndex::from_sources(WorkflowSources {
            active: snapshot,
            archive: ArchiveSnapshot::empty(),
        });
        Self::from_index_with_root(workflow_dir, repo_root, index)
    }

    fn from_index_with_root(
        workflow_dir: PathBuf,
        repo_root: PathBuf,
        index: WorkflowIndex,
    ) -> Self {
        Self {
            workflow_dir,
            repo_root,
            index,
            selected_index: 0,
            view_scope: ViewScope::Active,
            archived_done_count: None,
            archive_loaded: false,
            archive_error: None,
            selected_index_archived: 0,
            last_refresh_error: None,
            preview_open: false,
            preview_scroll: 0,
            max_preview_scroll: Cell::new(usize::MAX),
            preview_scroll_x: 0,
            max_preview_scroll_x: Cell::new(usize::MAX),
            preview_viewport_height: Cell::new(0),
            preview_wrap: true,
            task_page_size: Cell::new(10),
            sort_mode: SortMode::default(),
            sync_status: None,
            split_percent_left: 50,
            split_percent_bottom: 30,
            divider_drag: false,
            content_rect: Cell::new(Rect::default()),
            list_rows_rect: Cell::new(Rect::default()),
            id_column_rect: Cell::new(Rect::default()),
            list_offset: Cell::new(0),
            preview_rect: Cell::new(Rect::default()),
        }
    }

    /// Deterministic reload seam: swap the active snapshot in-place while
    /// preserving selection by core entity slug with a clamped-index fallback.
    /// Leaves `view_scope` and the archived-view state untouched — the
    /// watcher-driven refresh only re-parses active items; archived items are
    /// invalidated so the next scope toggle reloads them.
    pub fn reload_from_snapshot(&mut self, snapshot: WorkflowSnapshot) {
        let index = WorkflowIndex::from_sources(WorkflowSources {
            active: snapshot,
            archive: ArchiveSnapshot::empty(),
        });
        self.reload_from_index(index);
    }

    pub fn reload_from_index(&mut self, index: WorkflowIndex) {
        let prior_slug = self
            .selected_item()
            .and_then(|entity| entity_slug(&entity.path));

        self.index = index;
        self.index.clear_entity_activities();
        // Invalidate archive view — a watcher-driven reload may have touched
        // `_archive/` too. Dropping the cached list forces a rescan the next
        // time the user toggles to archived scope.
        self.archived_done_count = None;
        self.archive_loaded = false;
        self.archive_error = None;

        if self.view_scope == ViewScope::Archived {
            self.ensure_archive_loaded();
        }

        let len = self.row_count();
        if len == 0 {
            self.set_scope_index(0);
        } else if let Some(slug) = prior_slug {
            let visible = self.visible_items();
            if let Some(pos) = visible
                .iter()
                .position(|entity| entity_slug(&entity.path).as_deref() == Some(slug.as_str()))
            {
                self.set_scope_index(pos);
            } else if self.selected_index() >= len {
                self.set_scope_index(len - 1);
            }
        } else if self.selected_index() >= len {
            self.set_scope_index(len - 1);
        }

        self.reset_preview_scroll();
        self.last_refresh_error = None;
    }

    /// FS-touching reload wrapper. On success, delegates to
    /// `reload_from_snapshot`. On parse error, retains the prior snapshot
    /// and records the error in `last_refresh_error`.
    pub fn reload(&mut self) -> Result<(), ParseError> {
        match WorkflowIndex::load(&self.workflow_dir, &self.repo_root) {
            Ok(mut index) => {
                index.mark_history_loading();
                self.reload_from_index(index);
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

    pub fn sync_status(&self) -> Option<&SyncStatus> {
        self.sync_status.as_ref()
    }

    pub fn set_sync_status(&mut self, status: SyncStatus) {
        self.sync_status = Some(status);
    }

    pub fn workflow_dir(&self) -> &Path {
        &self.workflow_dir
    }

    pub fn definition(&self) -> &spacetop_core::domain::WorkflowDefinition {
        self.index.definition()
    }

    pub fn storage(&self) -> &WorkflowStorage {
        self.index.storage()
    }

    pub fn topology_diagnostic(&self) -> Option<StateTopologyDiagnostic> {
        let WorkflowStorage::SplitRoot {
            expected_branch,
            disposition,
            ..
        } = self.storage()
        else {
            return None;
        };
        match disposition {
            StateCheckoutDisposition::Attached => None,
            StateCheckoutDisposition::Detached => Some(StateTopologyDiagnostic::Detached),
            StateCheckoutDisposition::WrongBranch { actual_branch } => {
                Some(StateTopologyDiagnostic::WrongBranch {
                    actual_branch: actual_branch.clone(),
                    expected_branch: expected_branch.clone(),
                })
            }
            StateCheckoutDisposition::Missing => Some(StateTopologyDiagnostic::Missing),
            StateCheckoutDisposition::ProbeFailed { reason } => {
                Some(StateTopologyDiagnostic::ProbeFailed {
                    reason: reason.clone(),
                })
            }
        }
    }

    pub fn index(&self) -> &WorkflowIndex {
        &self.index
    }

    pub fn history_worker_request(&self) -> Option<HistoryWorkerRequest> {
        HistoryWorkerRequest::from_paths(&self.workflow_dir, &self.repo_root)
    }

    pub fn apply_history_result(&mut self, result: HistoryWorkerResult) {
        if result.workflow_dir == self.workflow_dir {
            self.index.replace_history_result(result.result);
        }
    }

    pub fn session_activity_worker_request(&self) -> Option<SessionActivityWorkerRequest> {
        let entities = self.index.session_scan_entities();
        (!entities.is_empty()).then(|| {
            SessionActivityWorkerRequest::from_state(&self.workflow_dir, &self.repo_root, entities)
        })
    }

    pub fn apply_session_activity_result(&mut self, result: SessionActivityWorkerResult) {
        if result.workflow_dir != self.workflow_dir || result.repo_root != self.repo_root {
            return;
        }
        match result.result {
            Ok(report) => {
                self.index.replace_session_scan_report(report);
            }
            Err(err) => {
                self.index.set_session_scan_error(err.to_string());
            }
        }
    }

    pub fn snapshot(&self) -> WorkflowSnapshot {
        WorkflowSnapshot {
            definition: self.index.definition().clone(),
            items: self.index.query(EntityQuery {
                scope: QueryScope::Active,
                status: None,
                text: None,
                field_filters: Vec::new(),
                sort: EntitySort::Id,
            }),
            parse_errors: self.index.active_parse_errors().to_vec(),
        }
    }

    pub fn selected_index(&self) -> usize {
        match self.view_scope {
            ViewScope::Active => self.selected_index,
            ViewScope::Archived => self.selected_index_archived,
        }
    }

    pub fn selected_item(&self) -> Option<Entity> {
        self.visible_items().get(self.selected_index()).cloned()
    }

    /// Per-entity parse errors captured during the most recent active-scope
    /// load. Empty on the happy path. Surfaced by the UI as synthetic "broken"
    /// rows appended after the regular work items.
    pub fn parse_errors(&self) -> &[EntityParseError] {
        match self.view_scope {
            ViewScope::Active => self.index.active_parse_errors(),
            ViewScope::Archived if self.archive_loaded => self.index.archive_parse_errors(),
            ViewScope::Archived => &[],
        }
    }

    /// Resolve the current selection to a `SelectedRow`. Selections beyond
    /// `visible_items().len()` index into the synthetic broken rows that the
    /// UI appends after the work items. Returns `None` when no row exists at
    /// the current index (e.g., empty workflow with no parse errors).
    pub fn selected_row(&self) -> Option<SelectedRow> {
        let items = self.visible_items();
        let idx = self.selected_index();
        if let Some(item) = items.get(idx) {
            return Some(SelectedRow::Item(Box::new(item.clone())));
        }
        let broken_idx = idx.checked_sub(items.len())?;
        self.parse_errors()
            .get(broken_idx)
            .cloned()
            .map(SelectedRow::Broken)
    }

    /// Total number of selectable rows: work items + synthetic broken rows
    /// (active scope) or just archived items (archived scope).
    pub(crate) fn row_count(&self) -> usize {
        self.visible_items().len() + self.parse_errors().len()
    }

    pub fn view_scope(&self) -> ViewScope {
        self.view_scope
    }

    pub fn apply_config_defaults(&mut self, config: &SpacetopConfig) {
        self.sort_mode = match config.defaults.sort {
            DefaultSort::Id => SortMode::Id,
            DefaultSort::Status => SortMode::Status,
        };
        self.set_view_scope(match config.defaults.scope {
            DefaultScope::Active => ViewScope::Active,
            DefaultScope::Archived => ViewScope::Archived,
        });
    }

    pub fn apply_session(&mut self, saved: &WorkflowSession) {
        self.set_view_scope(match saved.scope {
            WorkflowScope::Active => ViewScope::Active,
            WorkflowScope::Archived => ViewScope::Archived,
        });
        if let Some(id) = &saved.selected_entity_id {
            self.select_visible_entity_by_id(id);
        }
    }

    pub fn to_workflow_session(&self) -> WorkflowSession {
        WorkflowSession {
            selected_entity_id: self.selected_item().map(|entity| entity.id),
            scope: match self.view_scope {
                ViewScope::Active => WorkflowScope::Active,
                ViewScope::Archived => WorkflowScope::Archived,
            },
        }
    }

    pub fn current_query_scope(&self) -> QueryScope {
        match self.view_scope {
            ViewScope::Active => QueryScope::Active,
            ViewScope::Archived => QueryScope::Archived,
        }
    }

    pub fn visible_items(&self) -> Vec<Entity> {
        self.index.query(EntityQuery {
            scope: self.current_query_scope(),
            status: None,
            text: None,
            field_filters: Vec::new(),
            sort: match self.view_scope {
                ViewScope::Archived => EntitySort::ArchiveDefault,
                ViewScope::Active => match self.sort_mode {
                    SortMode::Id => EntitySort::Id,
                    SortMode::Status => EntitySort::Status,
                },
            },
        })
    }

    pub(crate) fn select_visible_entity_by_id(&mut self, entity_id: &str) -> bool {
        let Some(index) = self
            .visible_items()
            .iter()
            .position(|entity| entity.id == entity_id)
        else {
            return false;
        };
        self.set_scope_index(index);
        true
    }

    pub fn sort_mode(&self) -> SortMode {
        self.sort_mode
    }

    /// Cycle the active-scope sort mode (Id -> Status -> Id). Preserves the
    /// current selection by core entity slug across the re-sort, mirroring the
    /// reload_from_snapshot pattern. No-op if there are no active items.
    pub fn cycle_sort_mode(&mut self) {
        let active_items = self.active_items();
        if active_items.is_empty() {
            self.selected_index = 0;
            return;
        }

        let prior_slug = self
            .active_items()
            .get(self.selected_index)
            .and_then(|item| entity_slug(&item.path));

        self.sort_mode = match self.sort_mode {
            SortMode::Id => SortMode::Status,
            SortMode::Status => SortMode::Id,
        };

        let items = self.active_items();
        let len = items.len();
        if len == 0 {
            self.selected_index = 0;
            return;
        }
        if let Some(slug) = prior_slug {
            if let Some(pos) = items
                .iter()
                .position(|item| entity_slug(&item.path).as_deref() == Some(slug.as_str()))
            {
                self.selected_index = pos;
                return;
            }
        }
        if self.selected_index >= len {
            self.selected_index = len - 1;
        }
    }

    fn active_items(&self) -> Vec<Entity> {
        self.index.query(EntityQuery {
            scope: QueryScope::Active,
            status: None,
            text: None,
            field_filters: Vec::new(),
            sort: match self.sort_mode {
                SortMode::Id => EntitySort::Id,
                SortMode::Status => EntitySort::Status,
            },
        })
    }

    pub fn archived_items(&self) -> Vec<Entity> {
        if !self.archive_loaded {
            return Vec::new();
        }
        self.index.query(EntityQuery {
            scope: QueryScope::Archived,
            status: None,
            text: None,
            field_filters: Vec::new(),
            sort: EntitySort::ArchiveDefault,
        })
    }

    pub fn archived_count(&self) -> Option<usize> {
        if self.archive_loaded {
            Some(self.archived_items().len())
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
        let archive = WorkflowSources::load_archive(&self.workflow_dir, self.index.definition());
        self.archive_error = archive.error.clone();
        self.archived_done_count = Some(count_archived_terminal_items(&archive.entities));
        self.index.replace_archive(archive);
        self.archive_loaded = true;
    }

    fn set_view_scope(&mut self, scope: ViewScope) {
        if scope == ViewScope::Archived {
            self.ensure_archive_loaded();
        }
        if self.view_scope != scope {
            self.view_scope = scope;
            self.reset_preview_scroll();
        }
        self.clamp_selection();
    }

    pub(crate) fn toggle_scope(&mut self) {
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
        match self.view_scope {
            ViewScope::Active => {
                let len = self.row_count();
                if len == 0 {
                    self.selected_index = 0;
                } else if self.selected_index >= len {
                    self.selected_index = len - 1;
                }
            }
            ViewScope::Archived => {
                let len = self.row_count();
                if len == 0 {
                    self.selected_index_archived = 0;
                } else if self.selected_index_archived >= len {
                    self.selected_index_archived = len - 1;
                }
            }
        }
    }

    pub fn stage_counts(&self) -> Vec<StageCount> {
        self.index.stage_counts(self.archived_done_count)
    }

    pub(crate) fn select_next(&mut self) {
        let len = self.row_count();
        if len == 0 {
            self.set_scope_index(0);
            return;
        }
        let next = (self.selected_index() + 1).min(len - 1);
        self.set_scope_index(next);
    }

    pub(crate) fn select_previous(&mut self) {
        let prev = self.selected_index().saturating_sub(1);
        self.set_scope_index(prev);
    }

    pub(crate) fn select_first(&mut self) {
        self.set_scope_index(0);
    }

    pub(crate) fn select_last(&mut self) {
        let last = self.row_count().saturating_sub(1);
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

    /// List-pane percent of the content area for the given placement.
    pub fn split_percent(&self, placement: PreviewPlacement) -> u16 {
        match placement {
            PreviewPlacement::Left => self.split_percent_left,
            PreviewPlacement::Bottom => self.split_percent_bottom,
        }
    }

    /// Set the list-pane percent for the given placement, clamped to
    /// [`SPLIT_PERCENT_MIN`]..=[`SPLIT_PERCENT_MAX`]. The ratio holds for
    /// the rest of the session (per tab; no persistence).
    pub(crate) fn set_split_percent(&mut self, placement: PreviewPlacement, percent: u16) {
        let clamped = percent.clamp(SPLIT_PERCENT_MIN, SPLIT_PERCENT_MAX);
        match placement {
            PreviewPlacement::Left => self.split_percent_left = clamped,
            PreviewPlacement::Bottom => self.split_percent_bottom = clamped,
        }
    }

    /// Open the preview if it is closed (idempotent, unlike
    /// [`Self::toggle_preview`]). Mouse-click convention: select + open in
    /// one action, without closing a preview that is already open.
    pub(crate) fn open_preview(&mut self) {
        if !self.preview_open {
            self.preview_open = true;
            self.reset_preview_scroll();
        }
    }

    /// Select the row at `index` (mouse click). Reuses the
    /// `set_scope_index` reset semantics, so moving to a different row
    /// resets the preview scroll exactly like keyboard navigation.
    pub(crate) fn select_row(&mut self, index: usize) {
        if index < self.row_count() {
            self.set_scope_index(index);
        }
    }

    /// Clamped wheel scroll of the preview body, `delta` rows (positive
    /// scrolls down). Thin wrapper keeping `scroll_preview_vertical` the
    /// single home for the clamp invariant.
    pub(crate) fn wheel_scroll_preview(&mut self, delta: isize) {
        self.scroll_preview_vertical(delta);
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

    /// Single home for the vertical scroll invariant: clamp the stored offset
    /// to `[0, max]` BEFORE applying the delta, then re-clamp. Clamping the
    /// stored field (not just the rendered value) is load-bearing — render
    /// borrows `&OverviewState` and can only clamp its local copy, so if a
    /// method left `preview_scroll > max` (e.g. after a resize shrank the doc)
    /// the next relative scroll would start from a poisoned offset. Positive
    /// delta scrolls down, negative up.
    fn scroll_preview_vertical(&mut self, delta: isize) {
        if !self.preview_open {
            return;
        }
        let max = self.max_preview_scroll.get();
        let cur = self.preview_scroll.min(max);
        self.preview_scroll = if delta >= 0 {
            cur.saturating_add(delta as usize).min(max)
        } else {
            cur.saturating_sub(delta.unsigned_abs())
        };
    }

    /// Page step in rows: one viewport minus a row of overlap for continuity,
    /// floored at 1 so a not-yet-measured or single-row viewport still moves.
    fn preview_page_step(&self) -> usize {
        self.preview_viewport_height.get().saturating_sub(1).max(1)
    }

    pub(crate) fn scroll_preview_down(&mut self) {
        let step = self.preview_page_step() as isize;
        self.scroll_preview_vertical(step);
    }

    pub(crate) fn scroll_preview_up(&mut self) {
        let step = self.preview_page_step() as isize;
        self.scroll_preview_vertical(-step);
    }

    pub(crate) fn scroll_preview_to_top(&mut self) {
        if !self.preview_open {
            return;
        }
        self.preview_scroll = 0;
    }

    /// Jump to the bottom. Reads the clamped `max` directly rather than setting
    /// `usize::MAX`: the MAX trick survives render-clamp for display but poisons
    /// the stored offset, so the next page-up would do `MAX - step` (still past
    /// bottom) and appear to do nothing.
    pub(crate) fn scroll_preview_to_bottom(&mut self) {
        if !self.preview_open {
            return;
        }
        self.preview_scroll = self.max_preview_scroll.get();
    }

    pub(crate) fn scroll_preview_right(&mut self) {
        if !self.preview_open {
            return;
        }
        let max = self.max_preview_scroll_x.get();
        self.preview_scroll_x = self.preview_scroll_x.saturating_add(8).min(max);
    }

    pub(crate) fn scroll_preview_left(&mut self) {
        if !self.preview_open {
            return;
        }
        self.preview_scroll_x = self.preview_scroll_x.saturating_sub(8);
    }

    pub(crate) fn page_selection_down(&mut self) {
        let len = self.row_count();
        if len == 0 {
            self.set_scope_index(0);
            return;
        }
        let step = self.task_page_size.get().max(1);
        let next = self.selected_index().saturating_add(step).min(len - 1);
        self.set_scope_index(next);
    }

    pub(crate) fn page_selection_up(&mut self) {
        let step = self.task_page_size.get().max(1);
        let prev = self.selected_index().saturating_sub(step);
        self.set_scope_index(prev);
    }

    fn reset_preview_scroll(&mut self) {
        self.preview_scroll = 0;
        self.max_preview_scroll.set(usize::MAX);
        self.preview_scroll_x = 0;
        self.max_preview_scroll_x.set(usize::MAX);
        // Render rewrites this each frame; reset to 0 keeps the page step at its
        // floor (1 row) for the gap before the next draw rather than a stale value.
        self.preview_viewport_height.set(0);
    }
}

fn count_archived_terminal_items(items: &[Entity]) -> usize {
    // Archive placement is the terminal signal. Older archived items may carry
    // `status: done`, while newer accepted items can preserve their pre-archive
    // gate status such as `review`.
    items.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacetop_core::domain::{StageDefinition, WorkflowDefinition};

    fn fixture_item(id: &str) -> Entity {
        Entity {
            path: PathBuf::from(format!("/tmp/{id}.md")),
            id: id.to_string(),
            title: format!("item {id}"),
            status: "design".to_string(),
            source: Some("test".to_string()),
            started: None,
            completed: None,
            verdict: None,
            score: None,
            worktree: None,
            issue: None,
            pr: None,
            body: "body".to_string(),
            worktree_source: None,
            main_body: None,
        }
    }

    fn fixture_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: PathBuf::from("/tmp/ow-test"),
                state: None,
                storage: Default::default(),
                stages: vec![StageDefinition {
                    name: "design".to_string(),
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
                stage_prose: HashMap::new(),
                transitions: Vec::new(),
            },
            items: vec![fixture_item("001")],
            parse_errors: Vec::new(),
        }
    }

    #[test]
    fn preview_wrap_default_on_for_loaded_overview() {
        let state = OverviewState::from_snapshot(PathBuf::from("/tmp/ow-test"), fixture_snapshot());
        assert!(
            state.preview_wrap(),
            "preview_wrap defaults to true at construction"
        );

        let mut state = state;
        state.toggle_preview();
        assert!(
            state.preview_wrap(),
            "preview_wrap remains true once preview is opened"
        );
    }

    #[test]
    fn preview_wrap_default_on_for_empty_overview() {
        let state = OverviewState::empty(PathBuf::from("/tmp/ow-empty"));
        assert!(state.preview_wrap());
    }

    fn stage(name: &str) -> StageDefinition {
        StageDefinition {
            name: name.to_string(),
            initial: false,
            terminal: false,
            gate: false,
            fresh: false,
            feedback_to: None,
            worktree: false,
            concurrency: None,
        }
    }

    fn item_with_status(id: &str, status: &str) -> Entity {
        let mut item = fixture_item(id);
        item.status = status.to_string();
        item
    }

    fn snapshot_with(items: Vec<Entity>, stages: Vec<StageDefinition>) -> WorkflowSnapshot {
        WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: PathBuf::from("/tmp/ow-test"),
                state: None,
                storage: Default::default(),
                stages,
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: HashMap::new(),
                stage_prose: HashMap::new(),
                transitions: Vec::new(),
            },
            items,
            parse_errors: Vec::new(),
        }
    }

    #[test]
    fn sort_by_id_orders_ascending_across_mixed_status() {
        let items = vec![
            item_with_status("010", "implement"),
            item_with_status("002", "design"),
            item_with_status("037", "plan"),
        ];
        let snap = snapshot_with(
            items,
            vec![stage("design"), stage("plan"), stage("implement")],
        );
        let state = OverviewState::from_snapshot(PathBuf::from("/tmp/ow-test"), snap);
        let ids: Vec<String> = state.visible_items().into_iter().map(|i| i.id).collect();
        assert_eq!(ids, vec!["002", "010", "037"]);
        assert_eq!(state.sort_mode(), SortMode::Id);
    }

    #[test]
    fn sort_by_status_uses_workflow_stage_order() {
        let items = vec![
            item_with_status("004", "done"),
            item_with_status("001", "implement"),
            item_with_status("002", "design"),
            item_with_status("003", "plan"),
            item_with_status("005", "design"),
        ];
        let snap = snapshot_with(
            items,
            vec![
                stage("design"),
                stage("plan"),
                stage("implement"),
                stage("done"),
            ],
        );
        let mut state = OverviewState::from_snapshot(PathBuf::from("/tmp/ow-test"), snap);
        state.cycle_sort_mode();
        assert_eq!(state.sort_mode(), SortMode::Status);
        let ids: Vec<String> = state.visible_items().into_iter().map(|i| i.id).collect();
        // design (002, 005), plan (003), implement (001), done (004); IDs ascending within stage.
        assert_eq!(ids, vec!["002", "005", "003", "001", "004"]);
    }

    #[test]
    fn sort_by_status_pushes_unknown_status_to_end() {
        let items = vec![
            item_with_status("001", "design"),
            item_with_status("002", "mystery"),
            item_with_status("003", "plan"),
        ];
        let snap = snapshot_with(items, vec![stage("design"), stage("plan")]);
        let mut state = OverviewState::from_snapshot(PathBuf::from("/tmp/ow-test"), snap);
        state.cycle_sort_mode();
        let ids: Vec<String> = state.visible_items().into_iter().map(|i| i.id).collect();
        assert_eq!(ids, vec!["001", "003", "002"]);
    }

    #[test]
    fn cycle_sort_mode_preserves_selection_by_slug() {
        let items = vec![
            item_with_status("010", "implement"),
            item_with_status("002", "design"),
            item_with_status("037", "plan"),
        ];
        let snap = snapshot_with(
            items,
            vec![stage("design"), stage("plan"), stage("implement")],
        );
        let mut state = OverviewState::from_snapshot(PathBuf::from("/tmp/ow-test"), snap);
        // Sorted by Id: ["002", "010", "037"]; select "010" at index 1.
        state.selected_index = 1;
        assert_eq!(state.selected_item().map(|i| i.id), Some("010".to_string()));
        state.cycle_sort_mode();
        assert_eq!(state.selected_item().map(|i| i.id), Some("010".to_string()));
    }

    #[test]
    fn cycle_sort_mode_default_and_cycles_back() {
        let state = OverviewState::from_snapshot(PathBuf::from("/tmp/ow-test"), fixture_snapshot());
        assert_eq!(state.sort_mode(), SortMode::Id);
        let mut state = state;
        state.cycle_sort_mode();
        assert_eq!(state.sort_mode(), SortMode::Status);
        state.cycle_sort_mode();
        assert_eq!(state.sort_mode(), SortMode::Id);
    }

    #[test]
    fn reload_from_snapshot_preserves_sort_mode() {
        let items = vec![
            item_with_status("010", "implement"),
            item_with_status("002", "design"),
        ];
        let snap = snapshot_with(items, vec![stage("design"), stage("implement")]);
        let mut state = OverviewState::from_snapshot(PathBuf::from("/tmp/ow-test"), snap);
        state.cycle_sort_mode();
        assert_eq!(state.sort_mode(), SortMode::Status);

        let reload_items = vec![
            item_with_status("020", "implement"),
            item_with_status("005", "design"),
        ];
        let reload_snap = snapshot_with(reload_items, vec![stage("design"), stage("implement")]);
        state.reload_from_snapshot(reload_snap);
        assert_eq!(state.sort_mode(), SortMode::Status);
        let ids: Vec<String> = state.visible_items().into_iter().map(|i| i.id).collect();
        assert_eq!(ids, vec!["005", "020"]);
    }

    #[test]
    fn cycling_sort_mode_does_not_mutate_snapshot_items() {
        let items = vec![
            item_with_status("010", "implement"),
            item_with_status("002", "design"),
            item_with_status("037", "plan"),
        ];
        let snap = snapshot_with(
            items.clone(),
            vec![stage("design"), stage("plan"), stage("implement")],
        );
        let mut state = OverviewState::from_snapshot(PathBuf::from("/tmp/ow-test"), snap);
        let original_ids: Vec<String> = state
            .snapshot()
            .items
            .iter()
            .map(|i| i.id.clone())
            .collect();
        for _ in 0..5 {
            state.cycle_sort_mode();
        }
        let after_ids: Vec<String> = state
            .snapshot()
            .items
            .iter()
            .map(|i| i.id.clone())
            .collect();
        assert_eq!(original_ids, after_ids);
        // The compatibility snapshot is query-backed and therefore id-sorted.
        assert_eq!(original_ids, vec!["002", "010", "037"]);
    }

    #[test]
    fn preview_wrap_persists_across_reload() {
        let mut state =
            OverviewState::from_snapshot(PathBuf::from("/tmp/ow-test"), fixture_snapshot());
        state.toggle_preview();
        state.toggle_preview_wrap();
        assert!(!state.preview_wrap(), "wrap toggled off");
        state.reload_from_snapshot(fixture_snapshot());
        assert!(
            !state.preview_wrap(),
            "wrap toggle persists across reload_from_snapshot"
        );
    }
}
