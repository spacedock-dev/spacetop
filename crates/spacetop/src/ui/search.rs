use ratatui::{
    layout::Rect,
    prelude::{Frame, Line, Span, Style},
    style::{Color, Modifier},
    widgets::{Block, Borders, Clear, Paragraph},
};
use spacetop_core::query::EntityQuery;

use crate::app::{matching_commands, OverviewSession, SearchMode, SearchState};

pub(super) fn render_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    session: &OverviewSession,
    state: &SearchState,
) {
    let popup = centered_rect(area, 72, 14);
    let title = match state.mode() {
        SearchMode::Search => "Search",
        SearchMode::Command => "Command",
    };
    let mut lines = vec![
        Line::from(Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("> {}", state.query())),
        Line::from(""),
    ];

    match state.mode() {
        SearchMode::Search => {
            let active = session.active_state();
            let rows = active.index().query(EntityQuery {
                scope: active.current_query_scope(),
                text: Some(state.query().to_string()),
                ..EntityQuery::default()
            });
            if rows.is_empty() {
                lines.push(Line::from("No matches"));
            } else {
                for (index, entity) in rows.into_iter().take(8).enumerate() {
                    lines.push(selectable_line(
                        index == state.selected_index(),
                        format!("{}  {}  {}", entity.id, entity.status, entity.title),
                    ));
                }
            }
        }
        SearchMode::Command => {
            let rows = matching_commands(state.query());
            if rows.is_empty() {
                lines.push(Line::from("No commands"));
            } else {
                for (index, command) in rows.into_iter().enumerate() {
                    lines.push(selectable_line(
                        index == state.selected_index(),
                        command.label.to_string(),
                    ));
                }
            }
        }
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black).fg(Color::White)),
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(paragraph, popup);
}

fn selectable_line(selected: bool, text: String) -> Line<'static> {
    if selected {
        Line::from(Span::styled(
            format!("> {text}"),
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from(format!("  {text}"))
    }
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}
