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
        "Metrics",
        Style::default().add_modifier(Modifier::BOLD),
    ))];

    match session.active_state().index().metrics() {
        Err(reason) => lines.push(Line::from(reason.user_message())),
        Ok(metrics) => {
            lines.push(Line::from(format!(
                "completed: {}",
                metrics.completed_entities
            )));
            lines.push(Line::from(format!(
                "throughput: {}",
                metrics.throughput_completed
            )));
            lines.push(Line::from(""));
            lines.push(Line::from("stage dwell"));
            for (stage, seconds) in sorted_map(metrics.stage_dwell_seconds) {
                lines.push(Line::from(format!("{stage:<12} {seconds}s")));
            }
            lines.push(Line::from(""));
            lines.push(Line::from("cycle time"));
            for (entity, seconds) in sorted_map(metrics.cycle_time_seconds) {
                lines.push(Line::from(format!("{entity:<12} {seconds}s")));
            }
            lines.push(Line::from(""));
            lines.push(Line::from("WIP"));
            let mut wip: Vec<_> = metrics.wip_by_stage.into_iter().collect();
            wip.sort_by(|a, b| a.0.cmp(&b.0));
            for (stage, count) in wip {
                lines.push(Line::from(format!("{stage:<12} {count}")));
            }
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title("Metrics")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black).fg(Color::White)),
        )
        .scroll((scroll.min(u16::MAX as usize) as u16, 0));
    frame.render_widget(paragraph, area);
}

fn sorted_map<T: Ord>(map: std::collections::HashMap<String, T>) -> Vec<(String, T)> {
    let mut rows: Vec<_> = map.into_iter().collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows
}
