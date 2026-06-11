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
    /// `D` from Overview: open the full-pane Workflow Definition view.
    OpenDefinition,
    /// `Y` from Overview: request a `git pull --ff-only` against the
    /// active workflow's repo root. Always emitted when the binding
    /// fires; the helper classifies availability and reports the result.
    RequestSync,
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
        // less/vim-style page scroll inside the preview body. Space/b page, g/G
        // jump to the ends. Gated on preview_open so the list-mode bindings (s,
        // D, and a future plain g) stay free when the preview is closed. Arrow
        // keys and j/k deliberately stay task navigation.
        KeyCode::Char(' ') if state.preview_open() => {
            state.scroll_preview_down();
            OverviewKeyAction::None
        }
        KeyCode::Char('b') if state.preview_open() => {
            state.scroll_preview_up();
            OverviewKeyAction::None
        }
        KeyCode::Char('g') if state.preview_open() => {
            state.scroll_preview_to_top();
            OverviewKeyAction::None
        }
        KeyCode::Char('G') if state.preview_open() => {
            state.scroll_preview_to_bottom();
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
            Some(item) => {
                let target = item
                    .worktree_source
                    .clone()
                    .unwrap_or_else(|| item.path.clone());
                OverviewKeyAction::OpenSelectedFile(target)
            }
            None => OverviewKeyAction::None,
        },
        KeyCode::Char('s') if !state.preview_open() => {
            state.cycle_sort_mode();
            OverviewKeyAction::None
        }
        KeyCode::Char('D') if !state.preview_open() => OverviewKeyAction::OpenDefinition,
        KeyCode::Char('Y') if !state.preview_open() => OverviewKeyAction::RequestSync,
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
    use spacetop_core::domain::{Entity, StageDefinition, WorkflowDefinition, WorkflowSnapshot};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn single_session_with_item(path: PathBuf) -> OverviewSession {
        single_session_with_item_worktree(path, None)
    }

    fn single_session_with_item_worktree(
        path: PathBuf,
        worktree_source: Option<PathBuf>,
    ) -> OverviewSession {
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
                stage_prose: HashMap::new(),
                transitions: Vec::new(),
            },
            items: vec![Entity {
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
                worktree_source,
                main_body: None,
            }],
            parse_errors: Vec::new(),
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

    /// AC-2 (worktree branch): `o` on an item with `worktree_source = Some(_)`
    /// emits an OpenSelectedFile intent carrying the worktree-resident path,
    /// not the main-branch path.
    #[test]
    fn o_with_worktree_source_opens_worktree_path() {
        let main_path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let worktree_path = PathBuf::from("/tmp/spacetop-keys-test/.worktrees/wt/task-001.md");
        let mut session =
            single_session_with_item_worktree(main_path.clone(), Some(worktree_path.clone()));
        session.active_state_mut().toggle_preview();
        assert!(session.active_state().preview_open());

        let action = handle_overview_key(&mut session, key(KeyCode::Char('o')));
        match action {
            OverviewKeyAction::OpenSelectedFile(path) => {
                assert_eq!(path, worktree_path);
                assert_ne!(path, main_path);
            }
            _ => panic!("expected OpenSelectedFile intent"),
        }
    }

    /// AC-2 (None branch): `o` on an item with `worktree_source = None`
    /// falls back to the main-branch path.
    #[test]
    fn o_without_worktree_source_opens_main_path() {
        let main_path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item_worktree(main_path.clone(), None);
        session.active_state_mut().toggle_preview();

        let action = handle_overview_key(&mut session, key(KeyCode::Char('o')));
        match action {
            OverviewKeyAction::OpenSelectedFile(path) => assert_eq!(path, main_path),
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

    /// AC-1 (task 041): `D` from an overview with preview closed
    /// emits `OpenDefinition`.
    #[test]
    fn d_from_overview_emits_open_definition_action() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);
        assert!(!session.active_state().preview_open());
        let action = handle_overview_key(&mut session, key(KeyCode::Char('D')));
        assert!(
            matches!(action, OverviewKeyAction::OpenDefinition),
            "D with preview closed must emit OpenDefinition"
        );
    }

    /// AC-1: `Y` from an overview with preview closed emits `RequestSync`.
    #[test]
    fn y_from_overview_emits_request_sync() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);
        assert!(!session.active_state().preview_open());
        let action = handle_overview_key(&mut session, key(KeyCode::Char('Y')));
        assert!(
            matches!(action, OverviewKeyAction::RequestSync),
            "Y with preview closed must emit RequestSync"
        );
    }

    /// AC-1: `Y` while the preview is open is a silent no-op so the
    /// binding doesn't collide with future preview shortcuts.
    #[test]
    fn y_with_preview_open_is_noop() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);
        session.active_state_mut().toggle_preview();
        assert!(session.active_state().preview_open());
        let action = handle_overview_key(&mut session, key(KeyCode::Char('Y')));
        assert!(
            matches!(action, OverviewKeyAction::None),
            "Y while preview is open must be a no-op"
        );
    }

    /// AC-1 (task 041): `D` while the preview is open is a silent no-op.
    #[test]
    fn d_with_preview_open_is_noop() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);
        session.active_state_mut().toggle_preview();
        assert!(session.active_state().preview_open());
        let action = handle_overview_key(&mut session, key(KeyCode::Char('D')));
        assert!(
            matches!(action, OverviewKeyAction::None),
            "D while preview is open must be a no-op"
        );
    }
}
