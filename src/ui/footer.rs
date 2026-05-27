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
    let broken_pill_style = Style::default().fg(Color::Red).bg(PILL_BG);
    let sep_style = Style::default();
    let mut spans: Vec<Span<'_>> = Vec::new();
    for (i, hint) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", sep_style));
        }
        let style = if hint.starts_with('\u{26A0}') {
            broken_pill_style
        } else {
            pill_style
        };
        spans.push(Span::styled(hint.clone(), style));
    }

    let para = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(para, area);
}

/// Build the ordered footer pill labels for the active session. The first
/// pill is the parse-error count (`⚠ N broken`) when any per-entity parse
/// failures are present; the remainder are the static key hints. Returned as
/// owned `String`s so the dynamic broken-count label can be formatted in.
pub(crate) fn status_footer_hints(session: &OverviewSession) -> Vec<String> {
    let preview_open = session.active_state().preview_open();
    let mut hints: Vec<String> = Vec::new();
    let broken_count = session.active_state().parse_errors().len();
    if broken_count > 0 {
        hints.push(format!("\u{26A0} {broken_count} broken"));
    }
    hints.push("?: help".to_string());
    if !preview_open && session.is_multi() {
        hints.push("\u{2190}/\u{2192}: switch workflow".to_string());
    }
    if session.is_multi() {
        hints.push("P: pick workflow".to_string());
    }
    hints.push("\u{23CE}: toggle preview".to_string());
    hints.push("a: archive".to_string());
    if preview_open {
        // One compact scroll pill advertises the real keys; the full vocabulary
        // (incl. \u{2190}/\u{2192} horizontal scroll) lives in the help popup so this
        // single center-aligned line stays within ~80 cols.
        hints.push("scroll: Space/b PgUp/Dn g/G".to_string());
        hints.push("w: word wrap".to_string());
    } else {
        hints.push("PgUp/PgDn: page list".to_string());
        hints.push("s: sort".to_string());
        hints.push("D: definition".to_string());
    }
    if preview_open {
        hints.push("o: open".to_string());
    }
    hints.push("q: quit".to_string());
    hints
}
