use ratatui::{
    layout::Rect,
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::App;

pub(super) fn render_help_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let is_multi = app.as_session().map(|s| s.is_multi()).unwrap_or(false);
    let preview_open = app
        .as_session()
        .map(|s| s.active_state().preview_open())
        .unwrap_or(false);
    let mut lines = vec![
        Line::from(Span::styled(
            "Spacetop keymap",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  Up / k         move selection up"),
        Line::from("  Down / j       move selection down"),
        Line::from("  Home           jump to first item"),
        Line::from("  End            jump to last item"),
        Line::from("  Enter          toggle preview mode"),
        Line::from("  a              toggle active / archived view"),
        Line::from("  s              cycle sort mode (when preview closed)"),
        Line::from("  /              search entities"),
        Line::from("  :              open command palette"),
        Line::from("  D              open workflow definition"),
        Line::from("  Y              sync workflow (git pull)"),
        Line::from("  ?              toggle this help popup"),
        Line::from("  Esc            close help"),
    ];
    if preview_open {
        lines.push(Line::from("  Space / PgDn   page preview down"));
        lines.push(Line::from("  b / PgUp       page preview up"));
        lines.push(Line::from("  g / G          preview top / bottom"));
        lines.push(Line::from("  w              toggle word wrap"));
        lines.push(Line::from("  o              open file in $EDITOR"));
    } else {
        lines.push(Line::from("  PageUp         page list up"));
        lines.push(Line::from("  PageDown       page list down"));
        lines.push(Line::from(
            "  T              entity timeline (preview closed)",
        ));
        lines.push(Line::from("  M              metrics view (preview closed)"));
        lines.push(Line::from(
            "  A              activity feed (preview closed)",
        ));
        lines.push(Line::from(
            "  R              entity relations (preview closed)",
        ));
    }
    if preview_open {
        lines.push(Line::from("  \u{2192} / Right     scroll preview right"));
        lines.push(Line::from("  \u{2190} / Left      scroll preview left"));
    } else if is_multi {
        lines.push(Line::from("  \u{2192} / Right     switch to next workflow"));
        lines.push(Line::from(
            "  \u{2190} / Left      switch to previous workflow",
        ));
    }
    if is_multi {
        lines.push(Line::from("  P              pick workflow"));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "press ? or Esc to close",
        Style::default().add_modifier(Modifier::DIM),
    )));

    // Size to content: line count + top/bottom borders, clamped to the screen.
    // Replaces the old hand-counted height constants, which silently clipped
    // the bottom of the popup whenever a keybind row was added.
    let popup_w = area.width.min(64);
    let popup_h = (lines.len() as u16 + 2).min(area.height);
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect {
        x,
        y,
        width: popup_w,
        height: popup_h,
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title("Help")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black).fg(Color::White)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, popup);
    frame.render_widget(paragraph, popup);
}
