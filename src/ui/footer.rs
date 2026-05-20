use ratatui::{
    layout::{Alignment, Rect},
    prelude::{Frame, Line, Span, Style},
    style::Color,
    widgets::Paragraph,
};

use crate::app::OverviewSession;

const PILL_BG: Color = Color::Rgb(59, 66, 82);

/// One-line status footer at the bottom of the dashboard. Each key hint is
/// rendered as a pill-style styled span with a subtle background. The exact
/// key list adapts to single vs multi sessions.
pub(super) fn render_status_footer(frame: &mut Frame<'_>, area: Rect, session: &OverviewSession) {
    let hints = status_footer_hints(session);
    let pill_style = Style::default().fg(Color::White).bg(PILL_BG);
    let sep_style = Style::default();
    let mut spans: Vec<Span<'_>> = Vec::new();
    for (i, hint) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", sep_style));
        }
        spans.push(Span::styled(*hint, pill_style));
    }

    let para = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(para, area);
}

fn status_footer_hints(session: &OverviewSession) -> Vec<&'static str> {
    let preview_open = session.active_state().preview_open();
    let mut hints: Vec<&str> = vec!["?: help"];
    if preview_open {
        hints.push("\u{2190}/\u{2192}: preview scroll");
    } else if session.is_multi() {
        hints.push("\u{2190}/\u{2192}: switch workflow");
    }
    if session.is_multi() {
        hints.push("P: pick workflow");
    }
    hints.push("\u{23CE}: toggle preview");
    hints.push("a: archive");
    if preview_open {
        hints.push("PgUp/PgDn: preview scroll");
        hints.push("w: word wrap");
    } else {
        hints.push("PgUp/PgDn: page list");
        hints.push("s: sort");
        hints.push("D: definition");
    }
    if preview_open {
        hints.push("o: open");
    }
    hints.push("q: quit");
    hints
}
