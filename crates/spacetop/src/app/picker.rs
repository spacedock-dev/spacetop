use std::cell::Cell;
use std::path::{Path, PathBuf};

use spacetop_core::discovery::DiscoveredWorkflow;

#[derive(Debug, Clone)]
pub struct PickerState {
    pub scan_root: PathBuf,
    pub workflows: Vec<DiscoveredWorkflow>,
    pub selected_index: usize,
    pub error: Option<String>,
    /// Viewport height (in rows) of the workflow list area. Updated by the
    /// renderer each frame so PageUp/PageDown can step by a screen of items.
    pub viewport_height: Cell<usize>,
    /// First visible index in the workflow list. Updated by the renderer to
    /// keep `selected_index` within `[scroll_offset, scroll_offset + viewport_height)`.
    pub scroll_offset: Cell<usize>,
}

impl PartialEq for PickerState {
    fn eq(&self, other: &Self) -> bool {
        self.scan_root == other.scan_root
            && self.workflows == other.workflows
            && self.selected_index == other.selected_index
            && self.error == other.error
    }
}

impl Eq for PickerState {}

impl PickerState {
    pub fn new(scan_root: PathBuf, workflows: Vec<DiscoveredWorkflow>) -> Self {
        Self {
            scan_root,
            workflows,
            selected_index: 0,
            error: None,
            viewport_height: Cell::new(10),
            scroll_offset: Cell::new(0),
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

    pub(crate) fn select_next(&mut self) {
        if self.workflows.is_empty() {
            self.selected_index = 0;
            return;
        }
        self.selected_index = (self.selected_index + 1).min(self.workflows.len() - 1);
    }

    pub(crate) fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    pub(crate) fn select_first(&mut self) {
        self.selected_index = 0;
    }

    pub(crate) fn select_last(&mut self) {
        self.selected_index = self.workflows.len().saturating_sub(1);
    }

    pub(crate) fn page_selection_down(&mut self) {
        if self.workflows.is_empty() {
            self.selected_index = 0;
            return;
        }
        let step = self.viewport_height.get().max(1);
        let last = self.workflows.len() - 1;
        self.selected_index = self.selected_index.saturating_add(step).min(last);
    }

    pub(crate) fn page_selection_up(&mut self) {
        if self.workflows.is_empty() {
            self.selected_index = 0;
            return;
        }
        let step = self.viewport_height.get().max(1);
        self.selected_index = self.selected_index.saturating_sub(step);
    }

    /// Update `scroll_offset` so the selected row stays inside a viewport of
    /// `viewport_height` rows. Called by the renderer.
    pub(crate) fn ensure_selection_visible(&self, viewport_height: usize) {
        let len = self.workflows.len();
        if len == 0 || viewport_height == 0 {
            self.scroll_offset.set(0);
            return;
        }
        let mut offset = self.scroll_offset.get();
        let selected = self.selected_index.min(len - 1);
        if selected < offset {
            offset = selected;
        } else if selected >= offset + viewport_height {
            offset = selected + 1 - viewport_height;
        }
        // Clamp so we don't scroll past the end when the list shrinks or
        // viewport grows.
        let max_offset = len.saturating_sub(viewport_height);
        if offset > max_offset {
            offset = max_offset;
        }
        self.scroll_offset.set(offset);
    }
}
