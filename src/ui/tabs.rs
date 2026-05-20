use ratatui::{
    layout::Rect,
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::Tabs,
};

use crate::app::OverviewSession;

pub(super) fn render_workflow_tabs_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    session: &OverviewSession,
) -> Rect {
    let active = session.active_index();
    let tabs = session.discovery().iter().enumerate().map(|(index, disc)| {
        let label = match &disc.title {
            Some(t) if !t.trim().is_empty() => t.clone(),
            _ => disc
                .root
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| disc.root.display().to_string()),
        };
        let style = if index == active {
            Style::default()
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        Line::from(Span::styled(label, style))
    });
    let inner = area;
    let widget = Tabs::new(tabs)
        .select(active)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled(
            "|",
            Style::default().add_modifier(Modifier::DIM),
        ));
    frame.render_widget(widget, area);
    Rect {
        x: inner.x,
        y: inner.y.saturating_add(1),
        width: inner.width,
        height: inner.height.saturating_sub(1),
    }
}
