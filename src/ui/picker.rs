use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::app::PickerState;

pub fn render_in(frame: &mut Frame<'_>, area: Rect, state: &PickerState) {
    let outer = Block::default()
        .title("Pick Workflow")
        .borders(Borders::ALL);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let has_error = state.error().is_some();
    let mut constraints = vec![Constraint::Length(3), Constraint::Min(1)];
    if has_error {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);
    let title_area = areas[0];
    let list_area = areas[1];
    let (error_area, footer_area) = if has_error {
        (Some(areas[2]), areas[3])
    } else {
        (None, areas[2])
    };

    frame.render_widget(title(state), title_area);
    render_list(frame, list_area, state);
    if let Some(error_area) = error_area {
        frame.render_widget(error_line(state), error_area);
    }
    frame.render_widget(footer(), footer_area);
}

fn title(state: &PickerState) -> Paragraph<'_> {
    let lines = vec![
        Line::from(Span::styled(
            "spacetop — pick a workflow",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("scan root: {}", state.scan_root().display()),
            Style::default().add_modifier(Modifier::DIM),
        )),
    ];
    Paragraph::new(lines).block(Block::default().borders(Borders::BOTTOM))
}

fn render_list(frame: &mut Frame<'_>, area: Rect, state: &PickerState) {
    if state.workflows().is_empty() {
        state.viewport_height.set(area.height as usize);
        state.scroll_offset.set(0);
        frame.render_widget(Paragraph::new(Line::from("(no workflows)")), area);
        return;
    }

    let viewport = area.height as usize;
    state.viewport_height.set(viewport);
    state.ensure_selection_visible(viewport);
    let offset = state.scroll_offset.get();
    let total = state.workflows().len();
    let scan_root = state.scan_root();

    let end = (offset + viewport).min(total);
    let lines: Vec<Line> = state
        .workflows()
        .iter()
        .enumerate()
        .skip(offset)
        .take(end.saturating_sub(offset))
        .map(|(index, workflow)| workflow_row(scan_root, workflow, index == state.selected_index()))
        .collect();
    frame.render_widget(Paragraph::new(lines), area);

    // Scrollbar only when overflow.
    if total > viewport && viewport > 0 {
        let max_offset = total - viewport;
        let mut sb_state = ScrollbarState::new(max_offset + 1).position(offset);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("\u{2502}"))
                .thumb_symbol("\u{2588}"),
            area,
            &mut sb_state,
        );
    }
}

fn workflow_row<'a>(
    scan_root: &std::path::Path,
    workflow: &crate::discovery::DiscoveredWorkflow,
    selected: bool,
) -> Line<'a> {
    let rel = workflow
        .root
        .strip_prefix(scan_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| workflow.root.display().to_string());
    let title = workflow.title.as_deref().unwrap_or("");
    let text = if title.is_empty() {
        format!(" {rel}")
    } else {
        format!(" {rel}  —  {title}")
    };
    if selected {
        Line::from(Span::styled(
            text,
            Style::default().add_modifier(Modifier::REVERSED),
        ))
    } else {
        Line::from(text)
    }
}

fn error_line(state: &PickerState) -> Paragraph<'_> {
    let msg = state.error().unwrap_or("");
    Paragraph::new(Line::from(Span::styled(
        msg.to_string(),
        Style::default().fg(Color::Red),
    )))
}

fn footer<'a>() -> Paragraph<'a> {
    Paragraph::new(Line::from(Span::styled(
        "↑/↓ or j/k: move · PgUp/PgDn: page · Enter: open · q/Esc: quit",
        Style::default().add_modifier(Modifier::DIM),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::PickerState;
    use crate::discovery::DiscoveredWorkflow;
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
    use std::path::PathBuf;

    fn state_with(n: usize) -> PickerState {
        let workflows = (0..n)
            .map(|i| DiscoveredWorkflow {
                root: PathBuf::from(format!("/scan-root/docs/w{i}")),
                title: Some(format!("Workflow {i}")),
            })
            .collect();
        PickerState::new(PathBuf::from("/scan-root"), workflows)
    }

    fn buffer_text(buffer: &Buffer) -> String {
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("")
    }

    fn row_strings(buffer: &Buffer) -> Vec<String> {
        let mut rows = Vec::new();
        for y in 0..buffer.area.height {
            let mut row = String::new();
            for x in 0..buffer.area.width {
                row.push_str(buffer[(x, y)].symbol());
            }
            rows.push(row);
        }
        rows
    }

    #[test]
    fn renders_workflow_rows_and_title() {
        let state = state_with(3);
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(rendered.contains("pick a workflow"));
        assert!(rendered.contains("Pick Workflow"));
        assert!(rendered.contains("/scan-root"));
        assert!(rendered.contains("docs/w0"));
        assert!(rendered.contains("docs/w1"));
        assert!(rendered.contains("docs/w2"));
        assert!(rendered.contains("Workflow 0"));
        assert!(rendered.contains("Enter: open"));
    }

    #[test]
    fn renders_selected_row_with_reverse_modifier() {
        let mut state = state_with(3);
        state.selected_index = 1;
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let buffer = terminal.backend().buffer();

        let rows: Vec<u16> = (0..buffer.area.height)
            .filter(|y| {
                let mut row = String::new();
                for x in 0..buffer.area.width {
                    row.push_str(buffer[(x, *y)].symbol());
                }
                row.contains("docs/w")
            })
            .collect();
        assert_eq!(rows.len(), 3, "expected 3 workflow rows");

        for (idx, y) in rows.iter().enumerate() {
            let style = buffer[(1, *y)].style();
            if idx == 1 {
                assert!(
                    style.add_modifier.contains(Modifier::REVERSED),
                    "selected row at y={y} missing REVERSED modifier"
                );
            } else {
                assert!(
                    !style.add_modifier.contains(Modifier::REVERSED),
                    "non-selected row at y={y} should not be REVERSED"
                );
            }
        }
    }

    #[test]
    fn renders_error_line_when_present() {
        let mut state = state_with(2);
        state.set_error("failed to load /nonexistent: boom".to_string());
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("failed to load /nonexistent: boom"));
    }

    #[test]
    fn renders_bordered_dialog() {
        let state = state_with(2);
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let buffer = terminal.backend().buffer();

        assert_ne!(buffer[(0, 0)].symbol(), " ");
        assert_ne!(buffer[(99, 0)].symbol(), " ");
    }

    // AC-1: selection scrolls the visible window when N > viewport.
    #[test]
    fn list_scrolls_to_keep_selection_visible() {
        // Small terminal so the list area is a few rows tall.
        // Inner height = 20 - 2 (outer borders) = 18; minus 3 (title) and 1
        // (footer) = 14 list rows. Make N comfortably larger.
        let mut state = state_with(40);
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();

        // Initially selected index 0 — top of list is visible.
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("docs/w0"));
        assert!(!rendered.contains("docs/w39"), "last item shouldn't fit");

        // Move selection past the bottom of the viewport and re-render.
        state.selected_index = 39;
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("docs/w39"),
            "selection at end must be visible after scroll"
        );
        assert!(
            !rendered.contains("docs/w0 "),
            "first item should have scrolled out of view"
        );
        // Scroll offset should be > 0 now.
        assert!(state.scroll_offset.get() > 0);

        // And the selected row carries the REVERSED modifier.
        let buffer = terminal.backend().buffer();
        let selected_visible = row_strings(buffer)
            .iter()
            .enumerate()
            .any(|(y, row)| {
                row.contains("docs/w39")
                    && buffer[(1, y as u16)].style().add_modifier.contains(Modifier::REVERSED)
            });
        assert!(selected_visible, "selected row should be drawn with REVERSED");

        // Move selection back to the top — top edge tracking.
        state.selected_index = 0;
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("docs/w0"));
        assert_eq!(state.scroll_offset.get(), 0);
    }

    // AC-2: scrollbar renders when N > H, omitted when N <= H.
    #[test]
    fn scrollbar_renders_only_when_list_overflows() {
        // Overflow: many items in a small area => scrollbar present.
        let state = state_with(40);
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let buffer = terminal.backend().buffer();
        // The scrollbar thumb uses U+2588 (full block); track uses U+2502.
        let has_thumb = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() == "\u{2588}");
        let has_track = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() == "\u{2502}");
        assert!(has_thumb, "scrollbar thumb should be drawn when N > H");
        assert!(has_track, "scrollbar track should be drawn when N > H");

        // Non-overflow: few items in a large area => no scrollbar.
        let state = state_with(3);
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let has_thumb = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() == "\u{2588}");
        assert!(!has_thumb, "scrollbar should be omitted when N <= H");
    }

    // Regression (PR #25 review): when selection reaches the end of the list,
    // the scrollbar thumb must reach the final row of the list area. Using
    // `ScrollbarState::new(max_offset)` (without the `+ 1`) clamps the thumb
    // one row short of the bottom — match the convention in src/ui/mod.rs.
    #[test]
    fn scrollbar_thumb_reaches_bottom_when_selection_at_end() {
        let mut state = state_with(40);
        let total = state.workflows().len();
        state.selected_index = total - 1;

        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let buffer = terminal.backend().buffer();

        // Inner height of the outer block is 18 rows (20 - 2 borders).
        // Layout: title=3, list=Min(1), footer=1. List area runs from y=4
        // (after outer top border + 3 title rows) through y=17 (footer at 18).
        let list_top: u16 = 4;
        let list_bottom: u16 = 17; // last list row before footer at y=18

        let thumb_rows: Vec<u16> = (0..buffer.area.height)
            .filter(|y| (0..buffer.area.width).any(|x| buffer[(x, *y)].symbol() == "\u{2588}"))
            .collect();
        assert!(!thumb_rows.is_empty(), "scrollbar thumb should be drawn");
        let thumb_max = *thumb_rows.iter().max().unwrap();
        assert!(
            thumb_max >= list_top && thumb_max <= list_bottom,
            "thumb_max={thumb_max} must lie in list area [{list_top}, {list_bottom}]"
        );
        assert_eq!(
            thumb_max, list_bottom,
            "thumb should reach the bottom row of the list area when selection is last; got {thumb_max}, expected {list_bottom}"
        );
    }

    // AC-2 (cont): scrollbar thumb position tracks the selected proportion.
    #[test]
    fn scrollbar_thumb_advances_with_selection() {
        let mut state = state_with(40);
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();

        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let thumb_y_top: Vec<u16> = (0..buffer.area.height)
            .filter(|y| {
                (0..buffer.area.width).any(|x| buffer[(x, *y)].symbol() == "\u{2588}")
            })
            .collect();
        assert!(!thumb_y_top.is_empty(), "thumb should render at top");

        state.selected_index = 39;
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let thumb_y_bot: Vec<u16> = (0..buffer.area.height)
            .filter(|y| {
                (0..buffer.area.width).any(|x| buffer[(x, *y)].symbol() == "\u{2588}")
            })
            .collect();
        assert!(!thumb_y_bot.is_empty(), "thumb should render at bottom");
        assert!(
            thumb_y_bot.iter().min() > thumb_y_top.iter().max(),
            "thumb should move down when selection advances ({thumb_y_top:?} -> {thumb_y_bot:?})"
        );
    }
}
