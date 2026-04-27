use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::discovery::resolve_scan_root;
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
    pub preview_wrap: bool,
    pub task_page_size: Cell<usize>,
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
            preview_wrap: false,
            task_page_size: Cell::new(10),
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
        self.archived_done_count = None;
        self.archive_loaded = false;
        self.archive_error = None;
        self.refresh_archived_done_count();

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
        match self.load_archive_items() {
            Ok(items) => {
                self.archived_done_count = Some(count_done_items(&items));
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
            .map(|mut count| {
                if count.name == "done" {
                    if let Some(archived_done_count) = self.archived_done_count {
                        count.items = archived_done_count;
                    }
                }
                count
            })
            .collect()
    }

    pub(crate) fn select_next(&mut self) {
        let len = self.visible_items().len();
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

    pub(crate) fn scroll_preview_down(&mut self) {
        if !self.preview_open {
            return;
        }
        let max = self.max_preview_scroll.get();
        self.preview_scroll = self.preview_scroll.saturating_add(6).min(max);
    }

    pub(crate) fn scroll_preview_up(&mut self) {
        if !self.preview_open {
            return;
        }
        self.preview_scroll = self.preview_scroll.saturating_sub(6);
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
        let len = self.visible_items().len();
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
        self.preview_wrap = false;
    }
}

impl OverviewState {
    fn load_archive_items(&self) -> Result<Vec<WorkItem>, ParseError> {
        let allowed_statuses = self
            .snapshot
            .definition
            .stages
            .iter()
            .map(|stage| stage.name.clone())
            .collect::<Vec<_>>();
        load_archived_items(&self.workflow_dir, &allowed_statuses)
    }

    fn refresh_archived_done_count(&mut self) {
        match self.load_archive_items() {
            Ok(items) => {
                self.archived_done_count = Some(count_done_items(&items));
                self.archive_error = None;
            }
            Err(err) => {
                self.archived_done_count = Some(0);
                self.archive_error = Some(err.to_string());
            }
        }
    }
}

fn count_done_items(items: &[WorkItem]) -> usize {
    items.iter().filter(|item| item.status == "done").count()
}
