use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};

use crate::discovery::DiscoveredWorkflow;
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
    pub snapshot: WorkflowSnapshot,
    pub selected_index: usize,
    pub view_scope: ViewScope,
    pub archived_items: Vec<WorkItem>,
    pub archive_loaded: bool,
    pub archive_error: Option<String>,
    pub selected_index_archived: usize,
}

impl OverviewState {
    pub fn empty(workflow_dir: PathBuf) -> Self {
        let snapshot = WorkflowSnapshot {
            definition: crate::domain::WorkflowDefinition {
                root: workflow_dir.clone(),
                stages: Vec::new(),
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
            },
            items: Vec::new(),
        };
        Self {
            workflow_dir,
            snapshot,
            selected_index: 0,
            view_scope: ViewScope::Active,
            archived_items: Vec::new(),
            archive_loaded: false,
            archive_error: None,
            selected_index_archived: 0,
        }
    }

    pub fn load(workflow_dir: PathBuf) -> Result<Self, ParseError> {
        let snapshot = load_workflow_dir(&workflow_dir)?;
        Ok(Self::from_snapshot(workflow_dir, snapshot))
    }

    pub fn from_snapshot(workflow_dir: PathBuf, snapshot: WorkflowSnapshot) -> Self {
        Self {
            workflow_dir,
            snapshot,
            selected_index: 0,
            view_scope: ViewScope::Active,
            archived_items: Vec::new(),
            archive_loaded: false,
            archive_error: None,
            selected_index_archived: 0,
        }
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
        match self.view_scope {
            ViewScope::Active => self.selected_index = value,
            ViewScope::Archived => self.selected_index_archived = value,
        }
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

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Picker(PickerState),
    Overview(OverviewState),
}

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    mode: AppMode,
    should_quit: bool,
}

impl App {
    pub fn new(workflow_dir: impl Into<PathBuf>) -> Self {
        Self {
            mode: AppMode::Overview(OverviewState::empty(workflow_dir.into())),
            should_quit: false,
        }
    }

    pub fn load(workflow_dir: PathBuf) -> Result<Self, ParseError> {
        Ok(Self {
            mode: AppMode::Overview(OverviewState::load(workflow_dir)?),
            should_quit: false,
        })
    }

    pub fn from_snapshot(workflow_dir: PathBuf, snapshot: WorkflowSnapshot) -> Self {
        Self {
            mode: AppMode::Overview(OverviewState::from_snapshot(workflow_dir, snapshot)),
            should_quit: false,
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
        }
    }

    pub fn mode(&self) -> &AppMode {
        &self.mode
    }

    pub fn as_overview(&self) -> Option<&OverviewState> {
        match &self.mode {
            AppMode::Overview(state) => Some(state),
            _ => None,
        }
    }

    pub fn as_picker(&self) -> Option<&PickerState> {
        match &self.mode {
            AppMode::Picker(state) => Some(state),
            _ => None,
        }
    }

    // --- Back-compat accessors so existing overview tests keep compiling. ---

    fn overview(&self) -> &OverviewState {
        match &self.mode {
            AppMode::Overview(state) => state,
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

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match &mut self.mode {
            AppMode::Overview(state) => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Down | KeyCode::Char('j') => state.select_next(),
                KeyCode::Up | KeyCode::Char('k') => state.select_previous(),
                KeyCode::Home => state.select_first(),
                KeyCode::End => state.select_last(),
                KeyCode::Char('a') => state.toggle_scope(),
                _ => {}
            },
            AppMode::Picker(state) => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Down | KeyCode::Char('j') => state.select_next(),
                KeyCode::Up | KeyCode::Char('k') => state.select_previous(),
                KeyCode::Home => state.select_first(),
                KeyCode::End => state.select_last(),
                KeyCode::Enter => {
                    let Some(selected) = state.selected().cloned() else {
                        return;
                    };
                    state.clear_error();
                    match OverviewState::load(selected.root.clone()) {
                        Ok(overview) => {
                            self.mode = AppMode::Overview(overview);
                        }
                        Err(err) => {
                            state.set_error(format!(
                                "failed to load {}: {err}",
                                selected.root.display()
                            ));
                        }
                    }
                }
                _ => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{App, AppMode, ViewScope};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root.clone()).expect("workflow should load");
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
        assert_eq!(
            app.selected_item().map(|item| item.title.as_str()),
            Some("Build Initial TUI Overview")
        );
        assert_eq!(
            app.selected_item().map(|item| item.status.as_str()),
            app.snapshot()
                .items
                .first()
                .map(|item| item.status.as_str())
        );
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
}
