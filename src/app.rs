use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};

use crate::domain::{WorkItem, WorkflowSnapshot};
use crate::parser::load_workflow_dir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageCount {
    pub name: String,
    pub items: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    workflow_dir: PathBuf,
    snapshot: WorkflowSnapshot,
    selected_index: usize,
    should_quit: bool,
}

impl App {
    pub fn new(workflow_dir: impl Into<PathBuf>) -> Self {
        let workflow_dir = workflow_dir.into();
        Self {
            snapshot: WorkflowSnapshot {
                definition: crate::domain::WorkflowDefinition {
                    root: workflow_dir.clone(),
                    stages: Vec::new(),
                    id_style: None,
                    entity_type: None,
                    entity_label: None,
                    entity_label_plural: None,
                },
                items: Vec::new(),
            },
            workflow_dir,
            selected_index: 0,
            should_quit: false,
        }
    }

    pub fn load(workflow_dir: PathBuf) -> Result<Self, crate::parser::ParseError> {
        let snapshot = load_workflow_dir(&workflow_dir)?;
        Ok(Self::from_snapshot(workflow_dir, snapshot))
    }

    pub fn from_snapshot(workflow_dir: PathBuf, snapshot: WorkflowSnapshot) -> Self {
        Self {
            workflow_dir,
            snapshot,
            selected_index: 0,
            should_quit: false,
        }
    }

    pub fn workflow_dir(&self) -> &Path {
        &self.workflow_dir
    }

    pub fn snapshot(&self) -> &WorkflowSnapshot {
        &self.snapshot
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn selected_item(&self) -> Option<&WorkItem> {
        self.snapshot.items.get(self.selected_index)
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
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

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
            KeyCode::Home => self.selected_index = 0,
            KeyCode::End => self.select_last(),
            _ => {}
        }
    }

    fn select_next(&mut self) {
        if self.snapshot.items.is_empty() {
            self.selected_index = 0;
            return;
        }
        self.selected_index = (self.selected_index + 1).min(self.snapshot.items.len() - 1);
    }

    fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    fn select_last(&mut self) {
        self.selected_index = self.snapshot.items.len().saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::{Path, PathBuf};

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

        assert_eq!(app.workflow_dir(), root.as_path());
        assert_eq!(
            app.stage_counts()
                .iter()
                .map(|count| (count.name.as_str(), count.items))
                .collect::<Vec<_>>(),
            [
                ("design", 0),
                ("plan", 0),
                ("implement", 1),
                ("review", 0),
                ("done", 0)
            ]
        );
        assert_eq!(
            app.selected_item().map(|item| item.title.as_str()),
            Some("Build Initial TUI Overview")
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
