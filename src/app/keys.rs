use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};

use super::{OverviewSession, WorkflowSwitch};

pub(crate) enum OverviewKeyAction {
    None,
    OpenHelp,
    Quit,
    Switch(WorkflowSwitch),
    OpenPickerOverlay,
    OpenSelectedFile(PathBuf),
}

pub(crate) fn handle_overview_key(
    session: &mut OverviewSession,
    key: KeyEvent,
) -> OverviewKeyAction {
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
        KeyCode::Char('o') if state.preview_open() => match state.selected_item() {
            Some(item) => OverviewKeyAction::OpenSelectedFile(item.path.clone()),
            None => OverviewKeyAction::None,
        },
        KeyCode::Char('s') if !state.preview_open() => {
            state.cycle_sort_mode();
            OverviewKeyAction::None
        }
        KeyCode::Right if is_multi => OverviewKeyAction::Switch(session.cycle_next()),
        KeyCode::Left if is_multi => OverviewKeyAction::Switch(session.cycle_prev()),
        KeyCode::Char('P') if is_multi && !pinned => OverviewKeyAction::OpenPickerOverlay,
        _ => OverviewKeyAction::None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{handle_overview_key, OverviewKeyAction};
    use crate::app::{OverviewSession, OverviewState};
    use crate::domain::{StageDefinition, WorkItem, WorkflowDefinition, WorkflowSnapshot};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn single_session_with_item(path: PathBuf) -> OverviewSession {
        let root = PathBuf::from("/tmp/spacetop-keys-test");
        let snapshot = WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: root.clone(),
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
            items: vec![WorkItem {
                path,
                id: "001".to_string(),
                title: "T".to_string(),
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
                worktree_source: None,
                main_body: None,
            }],
        };
        let state = OverviewState::from_snapshot(root, snapshot);
        OverviewSession::single(state, true)
    }

    /// AC-1: `o` with preview open emits an OpenSelectedFile intent carrying
    /// the selected item's absolute path.
    #[test]
    fn o_with_preview_open_emits_open_file_intent() {
        let expected_path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(expected_path.clone());
        // Open the preview.
        session.active_state_mut().toggle_preview();
        assert!(session.active_state().preview_open());

        let action = handle_overview_key(&mut session, key(KeyCode::Char('o')));
        match action {
            OverviewKeyAction::OpenSelectedFile(path) => assert_eq!(path, expected_path),
            _ => panic!("expected OpenSelectedFile intent"),
        }
    }

    /// AC-3: `o` with preview closed is a silent no-op (no intent recorded).
    #[test]
    fn o_with_preview_closed_is_noop() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);
        assert!(!session.active_state().preview_open());

        let action = handle_overview_key(&mut session, key(KeyCode::Char('o')));
        assert!(
            matches!(action, OverviewKeyAction::None),
            "expected None action when preview is closed"
        );
    }
}
