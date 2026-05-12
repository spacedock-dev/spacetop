use crossterm::event::{KeyCode, KeyEvent};

use super::{OverviewSession, WorkflowSwitch};

pub(crate) enum OverviewKeyAction {
    None,
    OpenHelp,
    Quit,
    Switch(WorkflowSwitch),
    OpenPickerOverlay,
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
