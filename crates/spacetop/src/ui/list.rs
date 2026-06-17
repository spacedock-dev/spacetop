use ratatui::{
    layout::Rect,
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{Block, List, ListItem, ListState, Paragraph},
};

use crate::app::{OverviewState, ViewScope};
use spacetop_core::config::SpacetopConfig;
use spacetop_core::domain::{Entity, EntityParseError};

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

pub(super) fn render_task_list(
    frame: &mut Frame<'_>,
    area: Rect,
    config: &SpacetopConfig,
    state: &OverviewState,
) {
    let scope = state.view_scope();
    let title = match scope {
        ViewScope::Active => "Tasks",
        ViewScope::Archived => "Archived",
    };
    let block = Block::default();
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_items = state.visible_items();
    let item_count = visible_items.len();
    let items = build_task_list_items(state, &visible_items);

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
        // No rows drawn this frame: reset the hit-test facts so mouse
        // events cannot target rows from a previous, larger layout.
        state.list_rows_rect.set(Rect::default());
        state.list_offset.set(0);
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
        .highlight_style(Style::default().bg(crate::ui::color::selection_bg(config)));
    frame.render_stateful_widget(list, list_area, &mut list_state);

    // Render-facts for mouse hit-testing: the rows area actually drawn and
    // the scroll offset the List widget settled on (only observable after
    // the stateful render). Same Cell pattern as `task_page_size` above.
    state.list_rows_rect.set(list_area);
    state.list_offset.set(list_state.offset());
}

fn build_task_list_items(state: &OverviewState, items: &[Entity]) -> Vec<ListItem<'static>> {
    let scope = state.view_scope();
    let broken = state.parse_errors();
    if items.is_empty() && broken.is_empty() {
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

    // ID column width: widest visible ID, floored at 4 so numeric-ID
    // workflows (047, 048) are visually unchanged. No upper clamp — slug IDs
    // may be long and the Title simply starts further right.
    let icw = items
        .iter()
        .map(|item| item.id.chars().count())
        .max()
        .unwrap_or(4)
        .max(4);

    let mut rendered: Vec<ListItem<'_>> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            // Row format: "{gutter} {phase:<pcw} {id:>icw}  {title}"
            // Gutter: "▸ " for selected row, "  " otherwise (2 chars).
            // Phase column: user casing, pcw-char auto-sized width, ellipsized with "…" if longer.
            // ID: icw-char right-aligned, icw = max(4, longest visible ID).
            // Title: fills remaining width.
            let is_selected = index == selected_index && !items.is_empty();

            let gutter_text = if is_selected { "\u{25B8} " } else { "  " }; // "▸ " or "  "
            let gutter_style = if is_selected {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let id_str = format!("{:>width$}", item.id, width = icw);
            let phase = phase_col(&item.status, pcw);

            let id_style = Style::default().add_modifier(Modifier::DIM);
            let stage_color =
                crate::ui::color::to_color(state.definition().stage_color_for(&item.status));
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
            let (active_marker, active_marker_style) = if scope == ViewScope::Active
                && state.index().entity_has_active_session_marker(&item.id)
            {
                (
                    "@ ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ("  ", Style::default())
            };

            let mut spans: Vec<Span<'_>> = vec![
                Span::styled(gutter_text, gutter_style),
                Span::styled(phase, stage_style),
                Span::raw(" "),
                Span::styled(id_str, id_style),
                Span::raw("  "),
                Span::styled(active_marker, active_marker_style),
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
        .collect();

    // Append synthetic "broken" rows for entities whose frontmatter failed to
    // parse in the current scope. These rows are visually distinct (dim + red)
    // and never carry a worktree marker. Selection indices >= items.len()
    // target broken rows.
    let items_len = items.len();
    for (offset, err) in broken.iter().enumerate() {
        let index = items_len + offset;
        let is_selected = index == selected_index;
        rendered.push(broken_list_item(err, is_selected));
    }

    rendered
}

/// Render a synthetic "broken" entity row that surfaces a single parse
/// failure inline in the task list. The label format is `⚠ broken: <file>`;
/// the row is styled dim red so it does not blend with valid items.
pub(crate) fn broken_list_item(err: &EntityParseError, is_selected: bool) -> ListItem<'static> {
    let gutter_text = if is_selected { "\u{25B8} " } else { "  " };
    let gutter_style = if is_selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let file_name = err
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("<unknown>")
        .to_string();
    let label = format!("\u{26A0} broken: {file_name}");
    let label_style = Style::default().fg(Color::Red).add_modifier(Modifier::DIM);
    ListItem::new(Line::from(vec![
        Span::styled(gutter_text, gutter_style),
        Span::styled(label, label_style),
    ]))
}
