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
    scroll: usize,
) {
    let mut lines = vec![Line::from(Span::styled(
        "Activity",
        Style::default().add_modifier(Modifier::BOLD),
    ))];

    match session.active_state().index().activity(None) {
        Err(reason) => lines.push(Line::from(reason.user_message().to_string())),
        Ok(events) if events.is_empty() => lines.push(Line::from("No activity events")),
        Ok(events) => {
            lines.push(Line::from("entity      stage       commit"));
            for event in events {
                lines.push(Line::from(format!(
                    "{:<11} {:<11} {}",
                    event.entity_id,
                    event.event.to,
                    short_commit(&event.event.commit.0)
                )));
            }
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title("Activity")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black).fg(Color::White)),
        )
        .scroll((scroll.min(u16::MAX as usize) as u16, 0));
    frame.render_widget(paragraph, area);
}

fn short_commit(commit: &str) -> &str {
    commit.get(..8).unwrap_or(commit)
}
