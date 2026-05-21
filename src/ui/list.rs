use ratatui::{
    layout::Rect,
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{Block, List, ListItem, ListState, Paragraph},
};

use crate::app::{OverviewState, ViewScope};

/// Format a phase name into a fixed `width`-character column, preserving the
/// user's original casing exactly. Names longer than `width` chars are
/// truncated at `width-1` chars and suffixed with `…`; no additional glyphs
/// are introduced beyond that truncation ellipsis.
///
/// `width` is expected to be in the range [4, 12] (as produced by
/// `phase_col_width`), but the function works correctly for any width ≥ 1.
pub(crate) fn phase_col(stage: &str, width: usize) -> String {
    let char_count = stage.chars().count();
    if char_count > width {
        let truncated: String = stage.chars().take(width - 1).collect();
        format!("{truncated}\u{2026}") // (width-1) chars + "…" = width
    } else {
        // left-aligned, space-padded to `width`
        let pad = width - char_count;
        format!("{stage}{}", " ".repeat(pad))
    }
}

pub(super) fn render_task_list(frame: &mut Frame<'_>, area: Rect, state: &OverviewState) {
    let scope = state.view_scope();
    let title = match scope {
        ViewScope::Active => "Tasks",
        ViewScope::Archived => "Archived",
    };
    let block = Block::default();
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items = build_task_list_items(state);
    let item_count = state.visible_items().len();

    // Section header: "Tasks  ·  N" (or "Archived  ·  N") above the list.
    let section_header_text = format!("{}  \u{00B7}  {}", title, item_count);
    let section_header = Line::from(Span::styled(
        section_header_text,
        Style::default().add_modifier(Modifier::DIM),
    ));
    if inner.height > 0 {
        let header_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(section_header), header_area);
    }

    // Shift list down by 1 row for the section header.
    let list_area = if inner.height > 1 {
        Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: inner.height - 1,
        }
    } else {
        return;
    };

    state.task_page_size.set(list_area.height.max(1) as usize);

    let mut list_state = ListState::default().with_selected(if items.is_empty() {
        None
    } else {
        Some(state.selected_index())
    });
    // The ▸ gutter glyph is encoded in each ListItem span so unselected rows
    // stay aligned. highlight_style floods the entire selected row width with
    // the selection background — no manual trailing spacer needed.
    let list = List::new(items)
        .highlight_symbol("")
        .highlight_style(Style::default().bg(BG2));
    frame.render_stateful_widget(list, list_area, &mut list_state);
}

/// Tokyo Night selection/visual color for selected rows.
/// Provides a distinct blue-tinted contrast against the dark terminal background (~Rgb(26,27,38)).
const BG2: Color = Color::Rgb(40, 52, 84);

fn build_task_list_items(state: &OverviewState) -> Vec<ListItem<'_>> {
    let scope = state.view_scope();
    let items = state.visible_items();
    if items.is_empty() {
        let empty_text = match (scope, state.archive_error()) {
            (ViewScope::Archived, Some(err)) => format!("archive load failed: {err}"),
            (ViewScope::Archived, None) => "No archived items found.".to_string(),
            (ViewScope::Active, _) => "No work items found.".to_string(),
        };
        return vec![ListItem::new(Line::from(empty_text))];
    }
    let selected_index = state.selected_index();

    // Compute the phase column width from the longest visible status name,
    // clamped to [4, 12]. This is done once per render pass over all items.
    let pcw = items
        .iter()
        .map(|item| item.status.chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 12);

    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            // Row format: "{gutter} {phase:<pcw} {id:>4}  {title}"
            // Gutter: "▸ " for selected row, "  " otherwise (2 chars).
            // Phase column: user casing, pcw-char auto-sized width, ellipsized with "…" if longer.
            // ID: 4-char right-aligned.
            // Title: fills remaining width.
            let is_selected = index == selected_index && !items.is_empty();

            let gutter_text = if is_selected { "\u{25B8} " } else { "  " }; // "▸ " or "  "
            let gutter_style = if is_selected {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let id_str = format!("{:>4}", item.id);
            let phase = phase_col(&item.status, pcw);

            let id_style = Style::default().add_modifier(Modifier::DIM);
            let stage_color = state.snapshot().definition.stage_color_for(&item.status);
            let stage_style = Style::default().fg(stage_color);
            let title_style = if scope == ViewScope::Archived {
                Style::default().add_modifier(Modifier::DIM)
            } else if is_selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let dim_style = Style::default().add_modifier(Modifier::DIM);
            // Worktree marker: "⎇ " for rows sourced from a worktree (either
            // worktree-only or divergent from main), "  " otherwise. Kept as a
            // fixed-width 2-char column so titles stay aligned across rows.
            let (wt_marker, wt_marker_style) = if item.worktree_source.is_some() {
                ("\u{2387} ", dim_style)
            } else {
                ("  ", Style::default())
            };

            let mut spans: Vec<Span<'_>> = vec![
                Span::styled(gutter_text, gutter_style),
                Span::styled(phase, stage_style),
                Span::raw(" "),
                Span::styled(id_str, id_style),
                Span::raw("  "),
                Span::styled(wt_marker, wt_marker_style),
                Span::styled(item.title.clone(), title_style),
            ];

            if scope == ViewScope::Archived {
                let glyph = match item.verdict.as_deref() {
                    Some("PASSED") => " [\u{2713}]",
                    Some(_) => " [\u{2717}]",
                    None => " [?]",
                };
                spans.push(Span::styled(glyph, title_style));
            }

            ListItem::new(Line::from(spans))
        })
        .collect()
}
