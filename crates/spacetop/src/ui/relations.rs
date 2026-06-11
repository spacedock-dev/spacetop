use ratatui::{
    layout::Rect,
    prelude::{Frame, Line, Span, Style},
    style::{Color, Modifier},
    widgets::{Block, Borders, Paragraph},
};
use spacetop_core::relations::RelationView;

use crate::app::OverviewSession;

pub(super) fn render_in(
    frame: &mut Frame<'_>,
    area: Rect,
    session: &OverviewSession,
    entity_id: &str,
    scroll: usize,
) {
    let mut lines = vec![Line::from(Span::styled(
        "Relations",
        Style::default().add_modifier(Modifier::BOLD),
    ))];

    match session.active_state().index().entity_details(entity_id) {
        None => lines.push(Line::from("Entity not found")),
        Some(details) => {
            lines.push(Line::from(format!("{}  {}", details.id, details.title)));
            lines.push(Line::from(format!("status: {}", details.status)));
            if let Some(worktree) = details.worktree {
                lines.push(Line::from(format!("worktree: {worktree}")));
            }
            lines.push(Line::from(""));
            if details.relations.is_empty() {
                lines.push(Line::from("No relations"));
            } else {
                for relation in details.relations {
                    lines.push(Line::from(relation_row(relation)));
                }
            }
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title("Relations")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black).fg(Color::White)),
        )
        .scroll((scroll.min(u16::MAX as usize) as u16, 0));
    frame.render_widget(paragraph, area);
}

fn relation_row(relation: RelationView) -> String {
    match relation {
        RelationView::Issue { value } => format!("issue: {value}"),
        RelationView::PullRequest { value } => format!("pr: {value}"),
        RelationView::FeedbackStage { from, to } => format!("feedback-to: {from} -> {to}"),
    }
}
