use ratatui::{
    layout::Rect,
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::Paragraph,
};

use crate::app::{OverviewState, ViewScope};

pub(super) fn render_header_bar(frame: &mut Frame<'_>, area: Rect, state: &OverviewState) {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let scope = state.view_scope();
    let (badge_text, badge_style) = match scope {
        ViewScope::Active => (
            "[active]",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        ViewScope::Archived => (
            "[archived]",
            Style::default().add_modifier(Modifier::DIM | Modifier::BOLD),
        ),
    };

    let sort_badge_text = format!("[sort: {}]", state.sort_mode().label());
    let sort_key_hint = "(press s)";

    // Fixed portions of the header line (excluding path).
    // "Workflow " + badge + "  " + archived label + "(press a)" + "  "
    //   + sort badge + " " + "(press s)" + " "
    let archived_label = match state.archived_count() {
        Some(n) => format!("archived: {n}  "),
        None => "archived: ".to_string(),
    };
    let key_hint = "(press a)";
    let prefix_len = "Workflow ".chars().count()
        + badge_text.chars().count()
        + 2 // "  " gap
        + archived_label.chars().count()
        + key_hint.chars().count()
        + 2 // "  " gap before sort badge
        + sort_badge_text.chars().count()
        + 1 // " " between sort badge and hint
        + sort_key_hint.chars().count()
        + 1; // trailing space before path

    let full_path = state.workflow_dir().display().to_string();
    let available = (area.width as usize).saturating_sub(prefix_len);
    // Left-truncate path if it doesn't fit.
    let path_str: String = if full_path.chars().count() <= available {
        full_path.clone()
    } else if available > 1 {
        let skip = full_path.chars().count().saturating_sub(available - 1);
        let truncated: String = full_path.chars().skip(skip).collect();
        format!("\u{2026}{truncated}") // "…" + rest
    } else {
        "\u{2026}".to_string()
    };

    // Compute trailing space padding to fill the full area width so the
    // header bar occupies every terminal cell (avoids blank right-edge cells).
    let used = "Workflow ".chars().count()
        + badge_text.chars().count()
        + 2 // "  " gap
        + archived_label.chars().count()
        + key_hint.chars().count()
        + 2 // "  " gap before sort badge
        + sort_badge_text.chars().count()
        + 1 // " " between sort badge and hint
        + sort_key_hint.chars().count()
        + 1 // " " before path
        + path_str.chars().count();
    let trailing_spaces = (area.width as usize).saturating_sub(used);

    let line = Line::from(vec![
        Span::styled("Workflow ", dim),
        Span::styled(badge_text, badge_style),
        Span::raw("  "),
        Span::styled(archived_label, dim),
        Span::styled(key_hint, dim),
        Span::raw("  "),
        Span::styled(sort_badge_text, dim.add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(sort_key_hint, dim),
        Span::raw(" "),
        Span::styled(path_str, dim),
        Span::styled(" ".repeat(trailing_spaces), dim),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
