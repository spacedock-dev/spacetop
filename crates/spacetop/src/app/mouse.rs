//! Mouse hit-testing and event handling for the overview (task 057).
//!
//! Hit-testing reads only the render-fact rects (and list offset) the
//! render pass recorded into `OverviewState` Cells — the same values the
//! widgets were drawn with, in the same frame — so click coordinates
//! cannot drift from drawn rows by construction. Freshness rides the
//! event-loop invariant that `run_terminal` draws before it polls input.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

use super::keys::OverviewKeyAction;
use super::overview::{OverviewState, PreviewPlacement};
use super::OverviewSession;

/// Rows moved per wheel notch over the preview body.
const WHEEL_SCROLL_ROWS: isize = 3;

/// What an overview cell coordinate falls on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverviewHit {
    /// A selectable list row (work item or synthetic broken row), by
    /// absolute index into the visible rows.
    ListRow(usize),
    /// The grabbable band around the list/preview divider border.
    Divider,
    /// The preview pane body.
    Preview,
    /// Anything else: headers, the graph ribbon, blank space, footer.
    Chrome,
}

/// Pure hit-test from the render-facts. The divider band wins over the
/// panes it overlaps so it stays grabbable.
pub(crate) fn overview_hit(state: &OverviewState, column: u16, row: u16) -> OverviewHit {
    let pos = Position::new(column, row);
    if divider_band(state).is_some_and(|band| band.contains(pos)) {
        return OverviewHit::Divider;
    }
    if state.preview_rect.get().contains(pos) {
        return OverviewHit::Preview;
    }
    let rows = state.list_rows_rect.get();
    if rows.contains(pos) {
        let index = state.list_offset.get() + usize::from(row - rows.y);
        if index < state.row_count() {
            return OverviewHit::ListRow(index);
        }
    }
    OverviewHit::Chrome
}

/// Mouse-event peer to `handle_overview_key_with_keymap`, sharing the
/// [`OverviewKeyAction`] application path (every current arm returns
/// `None`; the enum keeps future mouse actions on the keyboard plumbing).
pub(crate) fn handle_overview_mouse(
    session: &mut OverviewSession,
    mouse: MouseEvent,
) -> OverviewKeyAction {
    let state = session.active_state_mut();
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            match overview_hit(state, mouse.column, mouse.row) {
                OverviewHit::ListRow(index) => {
                    // AC-1: one action — select the clicked row and open
                    // (or keep open) the preview.
                    state.select_row(index);
                    state.open_preview();
                }
                OverviewHit::Divider => state.divider_drag = true,
                OverviewHit::Preview | OverviewHit::Chrome => {}
            }
        }
        MouseEventKind::Drag(MouseButton::Left) if state.divider_drag => {
            // AC-3: continuous resize while the divider grab is held.
            drag_divider(state, mouse.column, mouse.row);
        }
        MouseEventKind::Up(_) => state.divider_drag = false,
        // AC-2: hover position decides the wheel target.
        MouseEventKind::ScrollDown => match overview_hit(state, mouse.column, mouse.row) {
            OverviewHit::Preview => state.wheel_scroll_preview(WHEEL_SCROLL_ROWS),
            OverviewHit::ListRow(_) => state.select_next(),
            OverviewHit::Divider | OverviewHit::Chrome => {}
        },
        MouseEventKind::ScrollUp => match overview_hit(state, mouse.column, mouse.row) {
            OverviewHit::Preview => state.wheel_scroll_preview(-WHEEL_SCROLL_ROWS),
            OverviewHit::ListRow(_) => state.select_previous(),
            OverviewHit::Divider | OverviewHit::Chrome => {}
        },
        _ => {}
    }
    OverviewKeyAction::None
}

/// Placement of the open preview, derived from the recorded rects: a
/// preview right of the content origin is Left placement (its divider is
/// the left border); one below is Bottom (divider is the top border).
/// `None` while the preview is closed (`preview_rect` is reset).
fn preview_placement_from_facts(state: &OverviewState) -> Option<PreviewPlacement> {
    let preview = state.preview_rect.get();
    if preview.width == 0 || preview.height == 0 {
        return None;
    }
    let content = state.content_rect.get();
    if preview.x > content.x {
        Some(PreviewPlacement::Left)
    } else if preview.y > content.y {
        Some(PreviewPlacement::Bottom)
    } else {
        None
    }
}

/// The grabbable divider band: the preview's border column/row widened by
/// one cell on each side.
fn divider_band(state: &OverviewState) -> Option<Rect> {
    let preview = state.preview_rect.get();
    Some(match preview_placement_from_facts(state)? {
        PreviewPlacement::Left => Rect {
            x: preview.x.saturating_sub(1),
            y: preview.y,
            width: 3,
            height: preview.height,
        },
        PreviewPlacement::Bottom => Rect {
            x: preview.x,
            y: preview.y.saturating_sub(1),
            width: preview.width,
            height: 3,
        },
    })
}

/// Recompute the active placement's split percent from the cursor position
/// relative to the content rect. The 10..=90 percent clamp lives in
/// `set_split_percent`; `split_content` applies geometric pane minimums on
/// top at render time.
fn drag_divider(state: &mut OverviewState, column: u16, row: u16) {
    let Some(placement) = preview_placement_from_facts(state) else {
        return;
    };
    let content = state.content_rect.get();
    let percent = match placement {
        PreviewPlacement::Left if content.width > 0 => {
            u32::from(column.saturating_sub(content.x)) * 100 / u32::from(content.width)
        }
        PreviewPlacement::Bottom if content.height > 0 => {
            u32::from(row.saturating_sub(content.y)) * 100 / u32::from(content.height)
        }
        _ => return,
    };
    state.set_split_percent(placement, percent.min(u32::from(u16::MAX)) as u16);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};

    use super::*;
    use crate::app::{App, PreviewPlacement};
    use spacetop_core::domain::{Entity, StageDefinition, WorkflowDefinition, WorkflowSnapshot};

    fn entity(id: &str, title: &str, body: &str) -> Entity {
        Entity {
            path: PathBuf::from(format!("/tmp/mouse-test/{id}.md")),
            id: id.to_string(),
            title: title.to_string(),
            status: "design".to_string(),
            source: None,
            started: None,
            completed: None,
            verdict: None,
            score: None,
            worktree: None,
            issue: None,
            pr: None,
            body: body.to_string(),
            worktree_source: None,
            main_body: None,
        }
    }

    /// App with `n` items; preview initially closed.
    fn fixture_app(n: usize, body: &str) -> App {
        let root = PathBuf::from("/tmp/mouse-test");
        let items = (0..n)
            .map(|i| entity(&format!("{i:03}"), &format!("Task number {i:03}"), body))
            .collect();
        let snapshot = WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: root.clone(),
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
                stage_colors: std::collections::HashMap::new(),
                stage_prose: std::collections::HashMap::new(),
                transitions: Vec::new(),
            },
            items,
            parse_errors: Vec::new(),
        };
        App::from_snapshot(root, snapshot)
    }

    /// Draw once through the real render path so the render-fact Cells are
    /// populated exactly as in production (TestBackend, no PTY/raw mode).
    fn draw(app: &App, width: u16, height: u16) {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| crate::ui::render(frame, app))
            .expect("draw");
    }

    fn mouse_at(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn long_body() -> String {
        (0..200)
            .map(|i| format!("body line {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    // AC-1: a single left-click on an entity row selects it and opens the
    // preview in one action.
    #[test]
    fn click_on_row_selects_and_opens_preview() {
        let mut app = fixture_app(6, "body");
        draw(&app, 100, 30);
        let state = app.as_overview().expect("overview");
        let rows = state.list_rows_rect.get();
        assert!(!state.preview_open());
        assert_eq!(state.list_offset.get(), 0);

        app.handle_mouse(mouse_at(
            MouseEventKind::Down(MouseButton::Left),
            rows.x + 3,
            rows.y + 2,
        ));

        let state = app.as_overview().expect("overview");
        assert_eq!(state.selected_index(), 2, "third visible row selected");
        assert!(state.preview_open(), "click opens the preview");

        // Clicking another row while the preview is open re-targets it
        // without closing.
        draw(&app, 100, 30);
        let rows = app.as_overview().expect("overview").list_rows_rect.get();
        app.handle_mouse(mouse_at(
            MouseEventKind::Down(MouseButton::Left),
            rows.x + 1,
            rows.y + 4,
        ));
        let state = app.as_overview().expect("overview");
        assert_eq!(state.selected_index(), 4);
        assert!(state.preview_open(), "preview stays open");
    }

    // AC-1: clicks on non-row chrome (graph ribbon, blank list space)
    // change nothing.
    #[test]
    fn click_on_chrome_changes_nothing() {
        let mut app = fixture_app(3, "body");
        draw(&app, 100, 30);
        let state = app.as_overview().expect("overview");
        let rows = state.list_rows_rect.get();
        assert_eq!(overview_hit(state, rows.x, 2), OverviewHit::Chrome);

        // Graph ribbon (row 2 sits inside the 7-row ribbon under the header).
        app.handle_mouse(mouse_at(MouseEventKind::Down(MouseButton::Left), 10, 2));
        // Blank list space below the last row (3 items, taller viewport).
        app.handle_mouse(mouse_at(
            MouseEventKind::Down(MouseButton::Left),
            rows.x + 1,
            rows.y + 10,
        ));

        let state = app.as_overview().expect("overview");
        assert_eq!(state.selected_index(), 0, "selection unchanged");
        assert!(!state.preview_open(), "preview stays closed");
    }

    // AC-2: the wheel scrolls the panel under the cursor — preview body at
    // preview coords (clamped), list selection at list coords.
    #[test]
    fn wheel_targets_panel_under_cursor() {
        let mut app = fixture_app(6, &long_body());
        app.handle_key(key(KeyCode::Enter)); // open preview
        draw(&app, 100, 30); // 100 > 2*30 → Left placement
        let state = app.as_overview().expect("overview");
        let preview = state.preview_rect.get();
        let max_scroll = state.max_preview_scroll.get();
        assert!(max_scroll > 0, "long body must overflow the preview");

        // Wheel down over the preview: +3 rows per notch.
        let (px, py) = (preview.x + 3, preview.y + 2);
        app.handle_mouse(mouse_at(MouseEventKind::ScrollDown, px, py));
        assert_eq!(app.as_overview().expect("overview").preview_scroll(), 3);
        app.handle_mouse(mouse_at(MouseEventKind::ScrollUp, px, py));
        assert_eq!(app.as_overview().expect("overview").preview_scroll(), 0);
        // Scrolling up at the top stays clamped at 0.
        app.handle_mouse(mouse_at(MouseEventKind::ScrollUp, px, py));
        assert_eq!(app.as_overview().expect("overview").preview_scroll(), 0);
        // Scrolling far down clamps at the recorded max.
        for _ in 0..(max_scroll) {
            app.handle_mouse(mouse_at(MouseEventKind::ScrollDown, px, py));
        }
        assert_eq!(
            app.as_overview().expect("overview").preview_scroll(),
            max_scroll,
            "wheel scroll clamps at max_preview_scroll"
        );

        // Wheel over the list moves the selection instead.
        draw(&app, 100, 30);
        let rows = app.as_overview().expect("overview").list_rows_rect.get();
        let (lx, ly) = (rows.x + 1, rows.y + 1);
        app.handle_mouse(mouse_at(MouseEventKind::ScrollDown, lx, ly));
        assert_eq!(app.as_overview().expect("overview").selected_index(), 1);
        app.handle_mouse(mouse_at(MouseEventKind::ScrollUp, lx, ly));
        assert_eq!(app.as_overview().expect("overview").selected_index(), 0);
    }

    // AC-2 guard: once the preview closes, its render-fact rect is reset,
    // so wheel events over the stale area no longer scroll anything.
    #[test]
    fn wheel_over_closed_preview_area_is_inert() {
        let mut app = fixture_app(6, &long_body());
        app.handle_key(key(KeyCode::Enter));
        draw(&app, 100, 30);
        let preview = app.as_overview().expect("overview").preview_rect.get();
        app.handle_key(key(KeyCode::Enter)); // close preview
        draw(&app, 100, 30);
        let state = app.as_overview().expect("overview");
        assert_eq!(state.preview_rect.get(), Rect::default());

        // The old preview area is now list space (full-width list) or
        // chrome — never a preview scroll.
        app.handle_mouse(mouse_at(
            MouseEventKind::ScrollDown,
            preview.x + 3,
            preview.y + 20,
        ));
        assert_eq!(app.as_overview().expect("overview").preview_scroll(), 0);
    }

    // AC-3 (Left placement): down → drag → up on the divider resizes the
    // split continuously, clamps at the edges, holds after release, and a
    // re-render honors the ratio.
    #[test]
    fn divider_drag_resizes_left_split() {
        let mut app = fixture_app(3, "body");
        app.handle_key(key(KeyCode::Enter));
        draw(&app, 100, 30); // Left placement
        let state = app.as_overview().expect("overview");
        assert_eq!(state.split_percent(PreviewPlacement::Left), 50);
        let preview = state.preview_rect.get();
        let content = state.content_rect.get();
        let grab_y = preview.y + 2;

        // Grab the divider border column.
        app.handle_mouse(mouse_at(
            MouseEventKind::Down(MouseButton::Left),
            preview.x,
            grab_y,
        ));
        assert!(app.as_overview().expect("overview").divider_drag);

        // Drag to 70% of the content width.
        let target_x = content.x + content.width * 7 / 10;
        app.handle_mouse(mouse_at(
            MouseEventKind::Drag(MouseButton::Left),
            target_x,
            grab_y,
        ));
        let state = app.as_overview().expect("overview");
        assert_eq!(state.split_percent(PreviewPlacement::Left), 70);
        // The Bottom ratio is untouched.
        assert_eq!(state.split_percent(PreviewPlacement::Bottom), 30);

        // Dragging past the right edge clamps to the 90% bound.
        app.handle_mouse(mouse_at(
            MouseEventKind::Drag(MouseButton::Left),
            content.x + content.width + 5,
            grab_y,
        ));
        assert_eq!(
            app.as_overview()
                .expect("overview")
                .split_percent(PreviewPlacement::Left),
            90
        );
        // And past the left edge clamps to 10%.
        app.handle_mouse(mouse_at(
            MouseEventKind::Drag(MouseButton::Left),
            content.x,
            grab_y,
        ));
        assert_eq!(
            app.as_overview()
                .expect("overview")
                .split_percent(PreviewPlacement::Left),
            10
        );

        // Release ends the drag; later drags are inert without a new grab.
        app.handle_mouse(mouse_at(MouseEventKind::Up(MouseButton::Left), content.x, grab_y));
        let state = app.as_overview().expect("overview");
        assert!(!state.divider_drag);
        app.handle_mouse(mouse_at(
            MouseEventKind::Drag(MouseButton::Left),
            target_x,
            grab_y,
        ));
        let state = app.as_overview().expect("overview");
        assert_eq!(
            state.split_percent(PreviewPlacement::Left),
            10,
            "drag without grab must not resize"
        );

        // A re-render honors the held ratio: list pane is 10% wide.
        draw(&app, 100, 30);
        let state = app.as_overview().expect("overview");
        let content = state.content_rect.get();
        let preview = state.preview_rect.get();
        assert_eq!(preview.x, content.x + content.width / 10);
    }

    // AC-3 (Bottom placement): the same drag sequence mutates the Bottom
    // ratio on a tall layout.
    #[test]
    fn divider_drag_resizes_bottom_split() {
        let mut app = fixture_app(3, "body");
        app.handle_key(key(KeyCode::Enter));
        draw(&app, 60, 40); // 60 <= 2*40 → Bottom placement
        let state = app.as_overview().expect("overview");
        assert_eq!(state.split_percent(PreviewPlacement::Bottom), 30);
        let preview = state.preview_rect.get();
        let content = state.content_rect.get();
        let grab_x = preview.x + 5;

        // Grab one cell above the border row (the widened band).
        app.handle_mouse(mouse_at(
            MouseEventKind::Down(MouseButton::Left),
            grab_x,
            preview.y - 1,
        ));
        assert!(app.as_overview().expect("overview").divider_drag);

        // Drag downwards to ~60% of the content height.
        let target_y = content.y + content.height * 6 / 10;
        app.handle_mouse(mouse_at(
            MouseEventKind::Drag(MouseButton::Left),
            grab_x,
            target_y,
        ));
        let expected =
            u16::try_from(u32::from(target_y - content.y) * 100 / u32::from(content.height))
                .expect("percent fits u16");
        let state = app.as_overview().expect("overview");
        assert_eq!(state.split_percent(PreviewPlacement::Bottom), expected);
        assert_eq!(
            state.split_percent(PreviewPlacement::Left),
            50,
            "Left ratio is untouched"
        );

        // Drag above the content top clamps to 10%.
        app.handle_mouse(mouse_at(MouseEventKind::Drag(MouseButton::Left), grab_x, 0));
        let state = app.as_overview().expect("overview");
        assert_eq!(state.split_percent(PreviewPlacement::Bottom), 10);

        app.handle_mouse(mouse_at(MouseEventKind::Up(MouseButton::Left), grab_x, 0));
        assert!(!app.as_overview().expect("overview").divider_drag);
    }

    // Scrolled-list regression: a click maps through the recorded
    // list_offset, so clicking the first VISIBLE row selects the offset
    // index, not index 0.
    #[test]
    fn click_respects_list_scroll_offset() {
        let mut app = fixture_app(40, "body");
        for _ in 0..35 {
            app.handle_key(key(KeyCode::Down));
        }
        draw(&app, 100, 30);
        let state = app.as_overview().expect("overview");
        let rows = state.list_rows_rect.get();
        let offset = state.list_offset.get();
        assert!(offset > 0, "fixture must scroll the list");

        app.handle_mouse(mouse_at(
            MouseEventKind::Down(MouseButton::Left),
            rows.x + 1,
            rows.y,
        ));
        let state = app.as_overview().expect("overview");
        assert_eq!(state.selected_index(), offset);
        assert!(state.preview_open());
    }

    // Mouse input is inert while the help popup is open, mirroring the
    // keyboard's consume_help_key guard.
    #[test]
    fn mouse_is_inert_while_help_is_open() {
        let mut app = fixture_app(3, "body");
        draw(&app, 100, 30);
        let rows = app.as_overview().expect("overview").list_rows_rect.get();
        app.handle_key(key(KeyCode::Char('?')));
        assert!(app.help_open());

        app.handle_mouse(mouse_at(
            MouseEventKind::Down(MouseButton::Left),
            rows.x + 1,
            rows.y + 1,
        ));

        let state = app.as_overview().expect("overview");
        assert_eq!(state.selected_index(), 0);
        assert!(!state.preview_open());
        assert!(app.help_open());
    }
}
