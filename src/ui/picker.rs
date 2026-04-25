use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{Block, Borders, Paragraph},
};

use crate::app::PickerState;

pub fn render_in(frame: &mut Frame<'_>, area: Rect, state: &PickerState) {
    let has_error = state.error().is_some();
    let mut constraints = vec![Constraint::Length(3), Constraint::Min(1)];
    if has_error {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    let title_area = areas[0];
    let list_area = areas[1];
    let (error_area, footer_area) = if has_error {
        (Some(areas[2]), areas[3])
    } else {
        (None, areas[2])
    };

    frame.render_widget(title(state), title_area);
    frame.render_widget(list(state), list_area);
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

fn list(state: &PickerState) -> Paragraph<'_> {
    if state.workflows().is_empty() {
        return Paragraph::new(Line::from("(no workflows)"));
    }
    let scan_root = state.scan_root();
    let lines: Vec<Line> = state
        .workflows()
        .iter()
        .enumerate()
        .map(|(index, workflow)| {
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
            if index == state.selected_index() {
                Line::from(Span::styled(
                    text,
                    Style::default().add_modifier(Modifier::REVERSED),
                ))
            } else {
                Line::from(text)
            }
        })
        .collect();
    Paragraph::new(lines)
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
        "↑/↓ or j/k: move · Enter: open · q/Esc: quit",
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

    #[test]
    fn renders_workflow_rows_and_title() {
        let state = state_with(3);
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
        terminal
            .draw(|frame| render_in(frame, frame.area(), &state))
            .unwrap();
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(rendered.contains("pick a workflow"));
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
}
