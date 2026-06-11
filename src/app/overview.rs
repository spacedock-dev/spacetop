use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::discovery::resolve_scan_root;
use crate::domain::{Entity, EntityParseError, WorkflowSnapshot};
use crate::parser::{load_archived_items, load_workflow_dir, ParseError};

/// Selection target in the task list — either a real work item or a synthetic
/// "broken" row representing an entity whose frontmatter failed to parse.
#[derive(Debug, Clone, Copy)]
pub enum SelectedRow<'a> {
    Item(&'a Entity),
    Broken(&'a EntityParseError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewScope {
    #[default]
    Active,
    Archived,
}

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

/// Compare two work-item ids: parse the entire id as `u64` and order
/// numerically when both sides parse, otherwise fall back to lexical
/// order. Workflow ids are pure numeric (e.g., "037"), so whole-string
/// parsing is sufficient; equal numeric values are tiebroken lexically
/// on the full id for stability.
fn compare_ids(a: &str, b: &str) -> std::cmp::Ordering {
    let an = a.parse::<u64>().ok();
    let bn = b.parse::<u64>().ok();
    match (an, bn) {
        (Some(x), Some(y)) => x.cmp(&y).then_with(|| a.cmp(b)),
        _ => a.cmp(b),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageCount {
    pub name: String,
    pub items: usize,
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
    /// The pull was attempted but failed; `message` is the trimmed
    /// stderr / synthesized reason from the helper.
    Failed { message: String },
    /// The repo state precluded a pull (no git repo, no upstream, no
    /// `origin` remote). `hint` is the user-facing description.
    Unavailable { hint: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverviewState {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub snapshot: WorkflowSnapshot,
    pub selected_index: usize,
    pub view_scope: ViewScope,
    pub archived_items: Vec<Entity>,
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
    pub sorted_active: Vec<Entity>,
    pub sync_status: Option<SyncStatus>,
}

/// Derive a stable slug from a work-item path. Prefer the file stem; when the
/// item lives in a folder-form `{slug}/index.md`, fall back to the parent
/// directory name so reload selection-preservation still matches across the
/// legacy and folder layouts.
pub(crate) fn slug_of(path: &Path) -> Option<String> {
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
                stage_prose: HashMap::new(),
                transitions: Vec::new(),
            },
            items: Vec::new(),
            parse_errors: Vec::new(),
        };
        Self {
            workflow_dir,
            repo_root,
            snapshot,
            selected_index: 0,
            view_scope: ViewScope::Active,
            archived_items: Vec::new(),
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
            sorted_active: Vec::new(),
            sync_status: None,
        }
    }

    pub fn load(workflow_dir: PathBuf) -> Result<Self, ParseError> {
        let repo_root = resolve_scan_root(&workflow_dir);
        let snapshot = load_workflow_dir(&workflow_dir, &repo_root)?;
        let mut state = Self::from_snapshot_with_root(workflow_dir, repo_root, snapshot);
        state.refresh_archived_done_count();
        Ok(state)
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
        let mut state = Self {
            workflow_dir,
            repo_root,
            snapshot,
            selected_index: 0,
            view_scope: ViewScope::Active,
            archived_items: Vec::new(),
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
            sorted_active: Vec::new(),
            sync_status: None,
        };
        state.rebuild_sorted_active();
        state
    }

    /// Deterministic reload seam: swap the active snapshot in-place while
    /// preserving selection by slug (file stem) with a clamped-index fallback.
    /// Leaves `view_scope` and the archived-view state untouched — the
    /// watcher-driven refresh only re-parses active items; archived items are
    /// invalidated so the next scope toggle reloads them.
    pub fn reload_from_snapshot(&mut self, snapshot: WorkflowSnapshot) {
        let prior_slug = self
            .sorted_active
            .get(self.selected_index)
            .and_then(|item| slug_of(&item.path));

        self.snapshot = snapshot;
        self.rebuild_sorted_active();
        // Invalidate archive view — a watcher-driven reload may have touched
        // `_archive/` too. Dropping the cached list forces a rescan the next
        // time the user toggles to archived scope.
        self.archived_items.clear();
        self.archived_done_count = None;
        self.archive_loaded = false;
        self.archive_error = None;
        self.refresh_archived_done_count();

        let len = self.sorted_active.len();
        if len == 0 {
            self.selected_index = 0;
        } else if let Some(slug) = prior_slug {
            if let Some(pos) = self
                .sorted_active
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

    pub fn sync_status(&self) -> Option<&SyncStatus> {
        self.sync_status.as_ref()
    }

    pub fn set_sync_status(&mut self, status: SyncStatus) {
        self.sync_status = Some(status);
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

    pub fn selected_item(&self) -> Option<&Entity> {
        self.visible_items().get(self.selected_index())
    }

    /// Per-entity parse errors captured during the most recent active-scope
    /// load. Empty on the happy path. Surfaced by the UI as synthetic "broken"
    /// rows appended after the regular work items.
    pub fn parse_errors(&self) -> &[EntityParseError] {
        match self.view_scope {
            ViewScope::Active => &self.snapshot.parse_errors,
            // Archived scope does not (yet) carry per-entity errors; the
            // archive loader still silently skips malformed entries.
            ViewScope::Archived => &[],
        }
    }

    /// Resolve the current selection to a `SelectedRow`. Selections beyond
    /// `visible_items().len()` index into the synthetic broken rows that the
    /// UI appends after the work items. Returns `None` when no row exists at
    /// the current index (e.g., empty workflow with no parse errors).
    pub fn selected_row(&self) -> Option<SelectedRow<'_>> {
        let items = self.visible_items();
        let idx = self.selected_index();
        if let Some(item) = items.get(idx) {
            return Some(SelectedRow::Item(item));
        }
        let broken_idx = idx.checked_sub(items.len())?;
        self.parse_errors().get(broken_idx).map(SelectedRow::Broken)
    }

    /// Total number of selectable rows: work items + synthetic broken rows
    /// (active scope) or just archived items (archived scope).
    pub(crate) fn row_count(&self) -> usize {
        self.visible_items().len() + self.parse_errors().len()
    }

    pub fn view_scope(&self) -> ViewScope {
        self.view_scope
    }

    pub fn visible_items(&self) -> &[Entity] {
        match self.view_scope {
            ViewScope::Active => &self.sorted_active,
            ViewScope::Archived => &self.archived_items,
        }
    }

    pub fn sort_mode(&self) -> SortMode {
        self.sort_mode
    }

    /// Cycle the active-scope sort mode (Id -> Status -> Id). Preserves the
    /// current selection by slug across the re-sort, mirroring the
    /// reload_from_snapshot pattern. No-op if there are no active items.
    pub fn cycle_sort_mode(&mut self) {
        if self.sorted_active.is_empty() {
            self.selected_index = 0;
            return;
        }

        let prior_slug = self
            .sorted_active
            .get(self.selected_index)
            .and_then(|item| slug_of(&item.path));

        self.sort_mode = match self.sort_mode {
            SortMode::Id => SortMode::Status,
            SortMode::Status => SortMode::Id,
        };
        self.rebuild_sorted_active();

        let len = self.sorted_active.len();
        if len == 0 {
            self.selected_index = 0;
            return;
        }
        if let Some(slug) = prior_slug {
            if let Some(pos) = self
                .sorted_active
                .iter()
                .position(|item| slug_of(&item.path).as_deref() == Some(slug.as_str()))
            {
                self.selected_index = pos;
                return;
            }
        }
        if self.selected_index >= len {
            self.selected_index = len - 1;
        }
    }

    fn rebuild_sorted_active(&mut self) {
        let mut items: Vec<Entity> = self.snapshot.items.clone();
        match self.sort_mode {
            SortMode::Id => {
                items.sort_by(|a, b| compare_ids(&a.id, &b.id));
            }
            SortMode::Status => {
                let stage_count = self.snapshot.definition.stages.len();
                let stage_index = |status: &str| -> usize {
                    self.snapshot
                        .definition
                        .stages
                        .iter()
                        .position(|s| s.name == status)
                        .unwrap_or(stage_count)
                };
                items.sort_by(|a, b| {
                    let ai = stage_index(&a.status);
                    let bi = stage_index(&b.status);
                    ai.cmp(&bi).then_with(|| compare_ids(&a.id, &b.id))
                });
            }
        }
        self.sorted_active = items;
    }

    pub fn archived_items(&self) -> &[Entity] {
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
        match self.load_archive_items() {
            Ok(items) => {
                self.archived_done_count = Some(count_archived_terminal_items(&items));
                self.archived_items = items;
                self.archive_error = None;
            }
            Err(err) => {
                self.archived_done_count = Some(0);
                self.archived_items = Vec::new();
                self.archive_error = Some(err.to_string());
            }
        }
        self.archive_loaded = true;
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
                let len = self.archived_items.len();
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
            .map(|mut count| {
                if count.name == "done" {
                    if let Some(archived_done_count) = self.archived_done_count {
                        count.items += archived_done_count;
                    }
                }
                count
            })
            .collect()
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

impl OverviewState {
    fn load_archive_items(&self) -> Result<Vec<Entity>, ParseError> {
        let allowed_statuses = self
            .snapshot
            .definition
            .stages
            .iter()
            .map(|stage| stage.name.clone())
            .collect::<Vec<_>>();
        let id_style = self.snapshot.definition.id_style.as_deref();
        load_archived_items(&self.workflow_dir, &allowed_statuses, id_style)
    }

    fn refresh_archived_done_count(&mut self) {
        match self.load_archive_items() {
            Ok(items) => {
                self.archived_done_count = Some(count_archived_terminal_items(&items));
                self.archive_error = None;
            }
            Err(err) => {
                self.archived_done_count = Some(0);
                self.archive_error = Some(err.to_string());
            }
        }
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
    use crate::domain::{StageDefinition, WorkflowDefinition};

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
        let ids: Vec<&str> = state
            .visible_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect();
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
        let ids: Vec<&str> = state
            .visible_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect();
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
        let ids: Vec<&str> = state
            .visible_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect();
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
        assert_eq!(state.selected_item().map(|i| i.id.as_str()), Some("010"));
        state.cycle_sort_mode();
        assert_eq!(state.selected_item().map(|i| i.id.as_str()), Some("010"));
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
        let ids: Vec<&str> = state
            .visible_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect();
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
        // Also verify the on-disk-derived order is the source-provided order.
        assert_eq!(original_ids, vec!["010", "002", "037"]);
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
