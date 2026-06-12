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
    let keymap = app.keymap();
    let mut lines = vec![
        Line::from(Span::styled(
            "Spacetop keymap",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        key_line("Up / k", "move selection up"),
        key_line("Down / j", "move selection down"),
        key_line("Home", "jump to first item"),
        key_line("End", "jump to last item"),
        key_line("Enter", "toggle preview mode"),
        key_line("a", "toggle active / archived view"),
        key_line("s", "cycle sort mode (when preview closed)"),
        key_line(keymap.search.label(), "search entities"),
        key_line(keymap.command.label(), "open command palette"),
        key_line("D", "open workflow definition"),
        key_line("Y", "sync workflow (git pull)"),
        key_line("?", "toggle this help popup"),
        key_line("Esc", "close help"),
    ];
    if preview_open {
        lines.push(key_line("Space / PgDn", "page preview down"));
        lines.push(key_line("b / PgUp", "page preview up"));
        lines.push(key_line("g / G", "preview top / bottom"));
        lines.push(key_line("w", "toggle word wrap"));
        lines.push(key_line("o", "open file in $EDITOR"));
    } else {
        lines.push(key_line("PageUp", "page list up"));
        lines.push(key_line("PageDown", "page list down"));
        lines.push(key_line(
            keymap.timeline.label(),
            "entity timeline (preview closed)",
        ));
        lines.push(key_line(
            keymap.metrics.label(),
            "metrics view (preview closed)",
        ));
        lines.push(key_line(
            keymap.activity.label(),
            "activity feed (preview closed)",
        ));
        lines.push(key_line(
            keymap.relations.label(),
            "entity relations (preview closed)",
        ));
    }
    if preview_open {
        lines.push(key_line("\u{2192} / Right", "scroll preview right"));
        lines.push(key_line("\u{2190} / Left", "scroll preview left"));
    } else if is_multi {
        lines.push(key_line("\u{2192} / Right", "switch to next workflow"));
        lines.push(key_line("\u{2190} / Left", "switch to previous workflow"));
    }
    if is_multi {
        lines.push(key_line("P", "pick workflow"));
    }
    // Mouse block (task 057). The Shift+drag line is load-bearing: the app
    // holds mouse capture for its lifetime and relies on the standard
    // terminal convention (iTerm2 / Terminal.app / kitty / WezTerm) that
    // Shift+left-drag bypasses capture for native selection/copy. The
    // string is pinned by a chrome test — update both together.
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Mouse",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(key_line("Click", "select row + open preview"));
    lines.push(key_line("Wheel", "scroll panel under cursor"));
    lines.push(key_line("Drag divider", "resize list/preview split"));
    lines.push(key_line("Shift+drag", "native terminal text selection"));
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

fn key_line(key: &str, description: &str) -> Line<'static> {
    Line::from(format!("  {key:<14} {description}"))
}
