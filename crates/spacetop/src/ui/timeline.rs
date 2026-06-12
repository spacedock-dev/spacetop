use ratatui::{
    layout::Rect,
    prelude::{Frame, Line, Span, Style},
    style::{Color, Modifier},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::OverviewSession;

pub(super) fn render_in(
    frame: &mut Frame<'_>,
    area: Rect,
    session: &OverviewSession,
    entity_id: &str,
    scroll: usize,
) {
    let mut lines = vec![Line::from(Span::styled(
        "Timeline",
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    match session.active_state().index().timeline(entity_id) {
        Err(reason) => lines.push(Line::from(reason.user_message())),
        Ok(events) if events.is_empty() => lines.push(Line::from("No timeline events")),
        Ok(events) => {
            lines.push(Line::from("stage       from        commit"));
            for event in events {
                lines.push(Line::from(format!(
                    "{:<11} {:<11} {}",
                    event.to,
                    event.from.unwrap_or_else(|| "-".to_string()),
                    short_commit(&event.commit.0)
                )));
            }
        }
    }
    render_lines(frame, area, lines, scroll);
}

fn short_commit(commit: &str) -> &str {
    commit.get(..8).unwrap_or(commit)
}

fn render_lines(frame: &mut Frame<'_>, area: Rect, lines: Vec<Line<'_>>, scroll: usize) {
    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title("Timeline")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black).fg(Color::White)),
        )
        .scroll((scroll.min(u16::MAX as usize) as u16, 0));
    frame.render_widget(paragraph, area);
}
