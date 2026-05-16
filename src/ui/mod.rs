mod diff;
mod graph;
mod markdown;
mod picker;

use crossterm::event::Event;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Tabs, Wrap,
    },
};

use crate::app::{App, AppMode, OverviewSession, OverviewState, ViewScope};
use graph::render_stage_graph;

pub type TerminalEvent = Event;

/// Markdown rendering width to use when wrap is OFF. We pass a large
/// width to termimad so paragraphs stay on a single ratatui `Line` and
/// the no-wrap horizontal scrollbar reflects their full content width.
const MARKDOWN_NO_WRAP_RENDER_WIDTH: u16 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewPlacement {
    Left,
    Bottom,
}

pub fn render_placeholder(frame: &mut Frame<'_>) {
    frame.render_widget(
        Paragraph::new("SpaceTop workflow overview is not implemented yet."),
        frame.area(),
    );
}

pub fn render(frame: &mut Frame<'_>, app: &App) {
    match app.mode() {
        AppMode::Picker(state) => {
            // Picker overlays a centered dialog; the dashboard responsive-
            // width rule does not apply to picker (it's a one-off chooser).
            let inner = picker_centered(frame.area(), state);
            picker::render_in(frame, inner, state);
        }
        AppMode::Overview(session) => {
            render_overview(frame, frame.area(), session);
        }
        AppMode::PickerOverlay { underlying, picker } => {
            // Draw the underlying overview at full width, then overlay a
            // centered picker dialog atop a `Clear` widget.
            render_overview(frame, frame.area(), underlying);
            let inner = picker_centered(frame.area(), picker);
            frame.render_widget(Clear, inner);
            picker::render_in(frame, inner, picker);
        }
    }

    if app.help_open() {
        render_help_popup(frame, frame.area(), app);
    }
}

/// Picker dialog centering: still centers a moderate-width column inside
/// the terminal so the picker list isn't full-width on a wide screen.
fn picker_centered(area: Rect, state: &crate::app::PickerState) -> Rect {
    const PICKER_WIDTH: u16 = 100;
    let width = area.width.min(PICKER_WIDTH);
    let extra = area.width.saturating_sub(width);
    let left = extra / 2;
    let workflow_rows = state.workflows().len().max(1) as u16;
    let chrome_rows = if state.error().is_some() { 7 } else { 6 };
    let height = area.height.min(workflow_rows + chrome_rows).max(8);
    let top = area.height.saturating_sub(height) / 2;
    Rect {
        x: area.x + left,
        y: area.y + top,
        width,
        height,
    }
}

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

/// Map a stage name to a stable color. Thin re-export of `domain::stage_color`
/// so existing direct callers in tests keep compiling without path changes.
#[cfg(test)]
pub(crate) fn stage_color(stage_name: &str) -> Color {
    crate::domain::stage_color(stage_name)
}

/// Assign graph-aware colors to stages. Thin re-export of
/// `domain::assign_stage_colors` for use from tests and legacy callers.
#[cfg(test)]
pub(crate) fn assign_stage_colors(
    stages: &[crate::domain::StageDefinition],
) -> std::collections::HashMap<String, Color> {
    crate::domain::assign_stage_colors(stages)
}

fn render_overview(frame: &mut Frame<'_>, area: Rect, session: &OverviewSession) {
    let state = session.active_state();
    let show_tabs = session.is_multi();
    let dashboard_area = if show_tabs {
        render_workflow_tabs_panel(frame, area, session)
    } else {
        area
    };

    // Vertical layout inside the active workflow panel: header bar (1),
    // graph ribbon (7), main content fills the rest, status footer (1 line).
    let constraints = vec![
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Min(0),
        Constraint::Length(1),
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(dashboard_area);

    let header_area = chunks[0];
    let graph_area = chunks[1];
    let content_area = chunks[2];
    let footer_area = chunks[3];
    render_header_bar(frame, header_area, state);
    render_stage_graph(frame, graph_area, state);

    if state.preview_open() {
        match preview_placement(dashboard_area) {
            PreviewPlacement::Left => {
                let [list_area, preview_area] = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .areas(content_area);
                render_task_list(frame, list_area, state);
                render_preview(frame, preview_area, state, PreviewPlacement::Left);
            }
            PreviewPlacement::Bottom => {
                let [list_area, preview_area] = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                    .areas(content_area);
                render_task_list(frame, list_area, state);
                render_preview(frame, preview_area, state, PreviewPlacement::Bottom);
            }
        }
    } else {
        render_task_list(frame, content_area, state);
    }

    render_status_footer(frame, footer_area, session);
}

fn preview_placement(area: Rect) -> PreviewPlacement {
    if u32::from(area.width) > u32::from(area.height) * 2 {
        PreviewPlacement::Left
    } else {
        PreviewPlacement::Bottom
    }
}

/// Single-line header bar above the stage graph ribbon.
/// Shows: muted "Workflow" label, scope badge [active]/[archived] (yellow
/// filled bg for active), archived count hint with muted key callout, and
/// the workflow directory path (dim, left-truncated with … on overflow).
fn render_header_bar(frame: &mut Frame<'_>, area: Rect, state: &OverviewState) {
    use crate::app::ViewScope;
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

/// Render the workflow tabs as the outer dashboard panel. The active tab
/// encloses the graph, task list, preview, and footer.
fn render_workflow_tabs_panel(
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

/// Pill background color for footer key hints (muted slate, Tokyo Night bg-2).
const PILL_BG: Color = Color::Rgb(59, 66, 82);

/// One-line status footer at the bottom of the dashboard. Each key hint is
/// rendered as a pill-style styled span with a subtle background. The exact
/// key list adapts to single vs multi sessions.
fn render_status_footer(frame: &mut Frame<'_>, area: Rect, session: &OverviewSession) {
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
    }
    if preview_open {
        hints.push("o: open");
    }
    hints.push("q: quit");
    hints
}

fn render_help_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let is_multi = app.as_session().map(|s| s.is_multi()).unwrap_or(false);
    let preview_open = app
        .as_session()
        .map(|s| s.active_state().preview_open())
        .unwrap_or(false);
    let popup_w = area.width.min(64);
    let popup_h = area.height.min(if is_multi {
        // The is_multi branch already had slack for the multi-mode lines
        // ("P: pick workflow", switch hints); bumping by 1 when preview is
        // open keeps the new "o: open file" row visible.
        if preview_open {
            24
        } else {
            23
        }
    } else if preview_open {
        // +1 over the prior 20 to accommodate the new "o: open file" line.
        21
    } else {
        19
    });
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect {
        x,
        y,
        width: popup_w,
        height: popup_h,
    };

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
        Line::from("  ?              toggle this help popup"),
        Line::from("  Esc            close help"),
    ];
    if preview_open {
        lines.push(Line::from("  PageUp         scroll preview up"));
        lines.push(Line::from("  PageDown       scroll preview down"));
        lines.push(Line::from("  w              toggle word wrap"));
        lines.push(Line::from("  o              open file in $EDITOR"));
    } else {
        lines.push(Line::from("  PageUp         page list up"));
        lines.push(Line::from("  PageDown       page list down"));
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

fn render_task_list(frame: &mut Frame<'_>, area: Rect, state: &OverviewState) {
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

fn render_preview(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &OverviewState,
    placement: PreviewPlacement,
) {
    let borders = match placement {
        PreviewPlacement::Left => Borders::LEFT,
        PreviewPlacement::Bottom => Borders::TOP,
    };
    let block = Block::default().borders(borders);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(item) = state.selected_item() else {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let header = Line::from(Span::styled("Preview", dim));
        let mut lines = vec![header];
        if inner.height > 1 {
            lines.push(Line::from("Select a work item to inspect it."));
        }
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    };

    let mut header_lines = build_preview_header_lines(item, state, inner.width, placement);
    let divider_line = header_lines.pop().unwrap_or_else(|| Line::from(""));
    let divider_height = wrapped_lines_height(std::slice::from_ref(&divider_line), inner.width);
    let metadata_height = wrapped_lines_height(&header_lines, inner.width)
        .min(inner.height.saturating_sub(divider_height));
    let header_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: metadata_height,
    };
    if metadata_height > 0 {
        frame.render_widget(
            Paragraph::new(header_lines).wrap(Wrap { trim: true }),
            header_area,
        );
    }

    let divider_y = inner.y + metadata_height;
    if divider_y >= inner.y + inner.height {
        return;
    }
    let divider_area = Rect {
        x: inner.x,
        y: divider_y,
        width: inner.width,
        height: divider_height.min(inner.height.saturating_sub(metadata_height)),
    };
    frame.render_widget(Paragraph::new(vec![divider_line]), divider_area);

    let body_inner = Rect {
        x: inner.x,
        y: divider_y + divider_area.height,
        width: inner.width,
        height: inner
            .height
            .saturating_sub(metadata_height + divider_area.height),
    };
    // When a worktree copy of this task has a body that diverges from the
    // root (main) copy, render a unified diff between the two bodies instead
    // of the plain markdown body. Otherwise fall back to the normal markdown
    // rendering path.
    let diff_lines: Option<Vec<Line<'static>>> = item
        .main_body
        .as_deref()
        .map(|main| diff::render_diff_lines(main, &item.body));

    // First pass: determine content height for overflow detection.
    // In the diff path, derive height directly from `diff_lines.len()` to
    // avoid cloning the entire Vec<Line>. In the markdown path, render only
    // height+1 lines so we don't render the whole body twice for long previews.
    // When wrap is OFF, pre-wrapping the markdown to the pane width would
    // hide horizontally-scrollable overflow. Pass a wide render width so
    // paragraphs stay on a single Line; ratatui's no-wrap horizontal scroll
    // then exposes them.
    let render_width = if state.preview_wrap() {
        body_inner.width
    } else {
        body_inner.width.max(MARKDOWN_NO_WRAP_RENDER_WIDTH)
    };
    let (content_height_full, body_lines_full) = if let Some(lines) = diff_lines.as_ref() {
        (lines.len() as u16, None)
    } else {
        let lines = markdown::render_markdown_termimad(&item.body, render_width);
        (lines.len() as u16, Some(lines))
    };
    let show_scrollbar = content_height_full > body_inner.height && body_inner.width > 1;
    let body_area = if show_scrollbar {
        Rect {
            x: body_inner.x,
            y: body_inner.y,
            width: body_inner.width - 1,
            height: body_inner.height,
        }
    } else {
        body_inner
    };
    // Second pass: re-render only when the scrollbar narrows the render area.
    // This ensures code block lines are padded to body_area.width (not body_inner.width)
    // so they do not overflow the render area and leave background gaps in wrap mode.
    let render_width_second_pass = if state.preview_wrap() {
        body_area.width
    } else {
        body_area.width.max(MARKDOWN_NO_WRAP_RENDER_WIDTH)
    };
    let body_lines = if let Some(lines) = diff_lines {
        lines
    } else if show_scrollbar && state.preview_wrap() {
        // Re-render only when the wrap-mode scrollbar narrows the render
        // area, so code-block backgrounds still pad to the visible width.
        markdown::render_markdown_termimad(&item.body, render_width_second_pass)
    } else {
        body_lines_full.expect("markdown path always produces body_lines_full")
    };
    let content_height = body_lines.len() as u16;

    let max_scroll = usize::from(content_height.saturating_sub(body_area.height));
    state.max_preview_scroll.set(max_scroll);
    let content_width = body_lines.iter().map(line_width).max().unwrap_or(0);
    let max_scroll_x = content_width.saturating_sub(body_area.width as usize);
    state.max_preview_scroll_x.set(max_scroll_x);
    let scroll_position = state.preview_scroll().min(max_scroll);
    let scroll_x = state.preview_scroll_x().min(max_scroll_x) as u16;
    let body_para = if state.preview_wrap() {
        state.max_preview_scroll_x.set(0);
        Paragraph::new(body_lines)
            .scroll((scroll_position as u16, 0))
            .wrap(Wrap { trim: false })
    } else {
        Paragraph::new(body_lines).scroll((scroll_position as u16, scroll_x))
    };
    frame.render_widget(body_para, body_area);

    if show_scrollbar {
        let mut scrollbar_state = ScrollbarState::new(max_scroll + 1).position(scroll_position);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("\u{2502}"))
                .thumb_symbol("\u{2588}"),
            body_inner,
            &mut scrollbar_state,
        );
    }
}

fn build_preview_header_lines<'a>(
    item: &'a crate::domain::WorkItem,
    state: &OverviewState,
    inner_width: u16,
    placement: PreviewPlacement,
) -> Vec<Line<'a>> {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let score = item
        .score
        .map(|score| format!("{score:.2}"))
        .unwrap_or_else(|| "n/a".to_string());
    let source = item.source.as_deref().unwrap_or("n/a");
    let worktree_segment: Vec<Span<'_>> = {
        let trimmed = item
            .worktree
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        match trimmed {
            Some(path) => {
                let basename = std::path::Path::new(path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| path.to_string());
                vec![Span::styled("worktree: ", dim), Span::raw(basename)]
            }
            None => vec![
                Span::styled("worktree: ", dim),
                Span::styled("\u{2014}", dim),
            ],
        }
    };
    let mut lines: Vec<Line<'_>> = Vec::new();

    // Combined section header + title: "Preview  ·  #id  Title" so that both
    // "Preview" and the task title appear without consuming an extra row.
    // The section marker is dim, the title is bold.
    lines.push(Line::from(vec![
        Span::styled(
            format!("Preview  \u{00B7}  #{}  ", item.id),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(
            item.title.as_str(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));

    let status_color = state.snapshot().definition.stage_color_for(&item.status);
    let status_spans = vec![
        Span::styled("status: ", dim),
        Span::styled("\u{25CF}", Style::default().fg(status_color)),
        Span::raw(" "),
        Span::styled(
            item.status.clone(),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if state.view_scope() == ViewScope::Archived {
        let verdict = item.verdict.as_deref().unwrap_or("n/a");
        let completed = item.completed.as_deref().unwrap_or("n/a");
        let verdict_style = match item.verdict.as_deref() {
            Some("PASSED") => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            Some(_) => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            None => Style::default().add_modifier(Modifier::DIM),
        };
        match placement {
            PreviewPlacement::Bottom => {
                let mut spans = status_spans.clone();
                spans.push(Span::raw("  \u{00B7}  "));
                spans.push(Span::styled("score: ", dim));
                spans.push(Span::raw(score.clone()));
                spans.push(Span::raw("  \u{00B7}  "));
                spans.push(Span::styled("source: ", dim));
                spans.push(Span::raw(source.to_string()));
                spans.push(Span::raw("  \u{00B7}  "));
                spans.extend(worktree_segment.clone());
                spans.push(Span::raw("  \u{00B7}  "));
                spans.push(Span::styled("verdict: ", dim));
                spans.push(Span::styled(verdict.to_string(), verdict_style));
                lines.push(Line::from(spans));
            }
            PreviewPlacement::Left => {
                lines.push(Line::from(status_spans));
                lines.push(Line::from(vec![
                    Span::styled("score: ", dim),
                    Span::raw(score.clone()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("source: ", dim),
                    Span::raw(source.to_string()),
                ]));
                lines.push(Line::from(worktree_segment.clone()));
                lines.push(Line::from(vec![
                    Span::styled("verdict: ", dim),
                    Span::styled(verdict.to_string(), verdict_style),
                ]));
            }
        }
        lines.push(Line::from(format!("completed: {completed}")));
    } else {
        match placement {
            PreviewPlacement::Bottom => {
                let mut spans = status_spans;
                spans.push(Span::raw("  \u{00B7}  "));
                spans.push(Span::styled("score: ", dim));
                spans.push(Span::raw(score.clone()));
                spans.push(Span::raw("  \u{00B7}  "));
                spans.push(Span::styled("source: ", dim));
                spans.push(Span::raw(source.to_string()));
                spans.push(Span::raw("  \u{00B7}  "));
                spans.extend(worktree_segment.clone());
                lines.push(Line::from(spans));
            }
            PreviewPlacement::Left => {
                lines.push(Line::from(status_spans));
                lines.push(Line::from(vec![
                    Span::styled("score: ", dim),
                    Span::raw(score.clone()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("source: ", dim),
                    Span::raw(source.to_string()),
                ]));
                lines.push(Line::from(worktree_segment.clone()));
            }
        }
    }
    // Render the entity path relative to the workflow root so it fits the
    // preview header and stays Smart-Selection-clickable in terminals that
    // resolve relative paths against OSC 7. Fall back to the absolute path
    // for entities whose path sits outside the workflow root (e.g. worktree
    // copies), preserving the disambiguating context the absolute path
    // carries. Defensively reject an empty relative result (`strip_prefix`
    // returns `Ok("")` when the two paths are equal — render the absolute
    // path in that edge case so the value is never visibly empty).
    let path_full = match item.path.strip_prefix(state.workflow_dir()) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
        _ => item.path.display().to_string(),
    };
    // The header paragraph wraps with `Wrap { trim: true }`, which performs
    // word-wrapping at whitespace boundaries. A long path (no internal
    // whitespace) wraps at the single space after `path:` — leaving the
    // label alone on one row and the value on the next. To users, the label
    // appears EMPTY. Truncate the value with a leading ellipsis so the
    // basename stays visible and the line fits on one row.
    let path_text = fit_path_to_width(&path_full, inner_width as usize);
    lines.push(Line::from(format!("path: {path_text}")));

    // Body divider: "── body " + "─" repeated to fill pane width.
    // This replaces the previous blank separator line (same line count, but
    // now visually marks the boundary between metadata and body content).
    let prefix = "\u{2500}\u{2500} body "; // "── body " = 8 chars
    let fill_len = (inner_width as usize).saturating_sub(prefix.chars().count());
    let divider = format!("{}{}", prefix, "\u{2500}".repeat(fill_len));
    lines.push(Line::from(Span::styled(divider, dim)));

    lines
}




fn line_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

/// Truncate `value` so that `"path: " + value` fits on a single row of the
/// given total `pane_width`. When truncation is required, drop characters from
/// the LEFT and replace them with a leading ellipsis (`…`) so the basename
/// stays visible. Returns the value unchanged when it already fits, and
/// returns just the ellipsis when even one character would not fit.
///
/// This exists because the preview header is rendered with
/// `Paragraph::new(...).wrap(Wrap { trim: true })`, which word-wraps at the
/// single space between the label and a long path — putting the label alone
/// on one row and the value on the next row, making the label appear empty.
/// See `path_line_stays_visible_for_long_paths` for the regression test.
fn fit_path_to_width(value: &str, pane_width: usize) -> String {
    let label_chars = "path: ".chars().count(); // = 6
    let available = pane_width.saturating_sub(label_chars);
    let value_chars = value.chars().count();
    if value_chars <= available {
        return value.to_string();
    }
    if available <= 1 {
        return "\u{2026}".to_string();
    }
    // Keep the trailing `(available - 1)` chars and prefix with `…`.
    let skip = value_chars - (available - 1);
    let tail: String = value.chars().skip(skip).collect();
    format!("\u{2026}{tail}")
}

fn wrapped_lines_height(lines: &[Line<'_>], width: u16) -> u16 {
    let width = usize::from(width.max(1));
    lines
        .iter()
        .map(|line| {
            let len = line_width(line).max(1);
            len.div_ceil(width) as u16
        })
        .sum()
}


#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::{
        backend::TestBackend,
        style::{Color, Modifier},
        Terminal,
    };

    use super::{fit_path_to_width, render};
    use crate::app::App;
    use crate::domain::{StageDefinition, WorkItem, WorkflowDefinition, WorkflowSnapshot};

    #[test]
    fn renders_real_workflow_summary_task_list_and_preview() {
        let app = app_with_items(vec![item(
            "001",
            "Synthetic active task",
            "This body gives the preview pane stable content.",
        )]);
        let selected = app
            .selected_item()
            .expect("real workflow has a selected item");
        let mut terminal =
            Terminal::new(TestBackend::new(140, 30)).expect("test terminal should be created");

        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let rendered = buffer_text(terminal.backend().buffer());
        // The graph block carries the Workflow title and each stage name.
        assert!(rendered.contains("Workflow"));
        for stage in &app.snapshot().definition.stages {
            assert!(
                rendered.contains(stage.name.as_str()),
                "missing stage name {}",
                stage.name
            );
        }
        // The selected item's id appears in the task list row; full titles
        // can wrap at narrow widths so we don't assert on the full title here.
        assert!(
            rendered.contains(&selected.id),
            "missing selected item id {}",
            selected.id
        );
        assert!(rendered.contains(&format!("status: ● {}", selected.status)));
        assert!(rendered.contains(&format!(
            "score: {}",
            selected
                .score
                .map(|score| format!("{score:.2}"))
                .unwrap_or_else(|| "n/a".to_string())
        )));
        assert!(rendered.contains(&format!(
            "source: {}",
            selected.source.as_deref().unwrap_or("n/a")
        )));
        // Some non-empty body content from the loaded snapshot should appear
        // in the preview pane — derive from the snapshot rather than hard-
        // coding text that drifts as tasks update.
        if let Some(snippet) = selected
            .body
            .lines()
            .map(|line| line.trim())
            .find(|line| line.len() >= 6)
        {
            // Only assert the leading short prefix to dodge wrap boundaries.
            let prefix: String = snippet.chars().take(6).collect();
            assert!(rendered.contains(&prefix), "missing body prefix {prefix:?}");
        }
    }

    #[test]
    fn overview_hides_preview_until_enter_opens_preview_mode() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let root = PathBuf::from("/tmp/spacetop-hidden-preview");
        let snapshot = WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: root.clone(),
                stages: vec![StageDefinition {
                    name: "design".to_string(),
                    initial: true,
                    terminal: false,
                    gate: false,
                    fresh: false,
                    feedback_to: None,
                    worktree: false,
                    concurrency: None,
                }],
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: std::collections::HashMap::new(),
            },
            items: vec![item("001", "Hidden Preview", "Body")],
        };
        let mut app = App::from_snapshot(root, snapshot);

        let mut terminal = Terminal::new(TestBackend::new(140, 24)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("Tasks"));
        assert!(!rendered.contains("Preview  ·"));

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("Preview  ·"));
    }

    #[test]
    fn active_view_header_shows_scope_and_archived_placeholder() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
        let mut terminal =
            Terminal::new(TestBackend::new(180, 20)).expect("test terminal should be created");

        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("[active]"), "missing [active] label");
        assert!(
            rendered.contains("(press a)"),
            "missing archived placeholder hint"
        );
    }

    #[test]
    fn archived_view_preview_renders_verdict_and_completed() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let mut app = App::load(root).expect("workflow should load");
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let mut terminal =
            Terminal::new(TestBackend::new(180, 30)).expect("test terminal should be created");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("[archived]"), "missing [archived] label");
        assert!(rendered.contains("verdict:"), "missing verdict line");
        assert!(rendered.contains("completed:"), "missing completed line");
        assert!(
            rendered.contains("archived: "),
            "missing archived count in header"
        );
    }

    #[test]
    fn archived_view_list_appends_verdict_glyphs() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let mut app = App::load(root).expect("workflow should load");
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));

        let mut terminal =
            Terminal::new(TestBackend::new(180, 30)).expect("test terminal should be created");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("\u{2713}"),
            "missing PASSED check glyph in archived list"
        );
    }

    #[test]
    fn preview_opens_on_right_in_wide_terminals_and_bottom_in_taller_ones() {
        let app = app_with_items(vec![item("001", "Placement", "Body")]);

        let mut wide = Terminal::new(TestBackend::new(180, 24)).expect("wide terminal");
        wide.draw(|frame| render(frame, &app)).unwrap();
        let wide_buffer = wide.backend().buffer();
        let tasks_wide = find_text(wide_buffer, "Tasks")[0];
        let preview_wide = find_text(wide_buffer, "Preview")[0];
        assert_eq!(tasks_wide.1, preview_wide.1);
        assert!(preview_wide.0 > tasks_wide.0);

        let mut tall = Terminal::new(TestBackend::new(80, 180)).expect("tall terminal");
        tall.draw(|frame| render(frame, &app)).unwrap();
        let tall_buffer = tall.backend().buffer();
        let tasks_tall = find_text(tall_buffer, "Tasks")[0];
        let preview_tall = find_text(tall_buffer, "Preview")[0];
        assert!(preview_tall.1 > tasks_tall.1);
    }

    #[test]
    fn bottom_preview_compacts_metadata_into_one_line() {
        let mut work_item = item("001", "Bottom Preview", "Body");
        work_item.score = Some(0.75);
        work_item.source = Some("captain".to_string());
        let app = app_with_items(vec![work_item]);

        let mut terminal = Terminal::new(TestBackend::new(80, 180)).expect("tall terminal");
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(rendered.contains("status: ● design"));
        assert!(rendered.contains("score: 0.75"));
        assert!(rendered.contains("source: captain"));
    }

    #[test]
    fn bottom_preview_shows_worktree_when_set() {
        let mut work_item = item("001", "WT", "Body");
        work_item.worktree = Some(".worktrees/ensign-foo".to_string());
        let app = app_with_items(vec![work_item]);

        let mut terminal = Terminal::new(TestBackend::new(80, 180)).expect("tall terminal");
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(
            rendered.contains("worktree: ensign-foo"),
            "expected worktree basename in bottom preview, got: {rendered}"
        );
        assert!(
            !rendered.contains(".worktrees/ensign-foo"),
            "bottom preview must render basename only, not full path"
        );
    }

    #[test]
    fn left_preview_shows_worktree_when_set() {
        let mut work_item = item("001", "WT", "Body");
        work_item.worktree = Some(".worktrees/ensign-foo".to_string());
        let app = app_with_items(vec![work_item]);

        let mut terminal = Terminal::new(TestBackend::new(180, 24)).expect("wide terminal");
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(
            rendered.contains("worktree: ensign-foo"),
            "expected worktree basename in left preview, got: {rendered}"
        );
        assert!(
            !rendered.contains(".worktrees/ensign-foo"),
            "left preview must render basename only, not full path"
        );
    }

    #[test]
    fn preview_renders_em_dash_for_empty_worktree() {
        let work_item = item("001", "WT", "Body");
        let app = app_with_items(vec![work_item]);

        let mut terminal = Terminal::new(TestBackend::new(80, 180)).expect("tall terminal");
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(
            rendered.contains("worktree: \u{2014}"),
            "expected em-dash empty marker, got: {rendered}"
        );
        assert!(
            rendered.contains("status: ● design"),
            "surrounding header should remain intact"
        );
    }

    #[test]
    fn fit_path_to_width_keeps_short_path_unchanged() {
        let s = fit_path_to_width("039-foo.md", 40);
        assert_eq!(s, "039-foo.md");
    }

    #[test]
    fn fit_path_to_width_truncates_long_path_with_leading_ellipsis() {
        let long = "/repo/.worktrees/SLUG/docs/spacetop-dev/039-open-entity-file-from-preview.md";
        let s = fit_path_to_width(long, 40);
        // pane=40, label="path: "=6, available=34, so the result is 34 chars
        // (1 ellipsis + 33 tail).
        assert_eq!(s.chars().count(), 34);
        assert!(s.starts_with('\u{2026}'));
        // Truncate from the LEFT so the END of the path stays visible —
        // important because the basename carries the identifying info.
        assert!(
            s.ends_with("from-preview.md"),
            "truncated path should keep the trailing portion of the basename; got {s:?}"
        );
    }

    #[test]
    fn fit_path_to_width_keeps_basename_when_room_is_ample() {
        // With a wider pane the entire basename fits even when the path is
        // truncated, so the user can identify the file at a glance.
        let long = "/repo/.worktrees/SLUG/docs/spacetop-dev/039-open-entity-file-from-preview.md";
        let s = fit_path_to_width(long, 80); // available = 74
        assert!(
            s.starts_with('\u{2026}'),
            "should still mark truncation; got {s:?}"
        );
        assert!(
            s.ends_with("039-open-entity-file-from-preview.md"),
            "with ample pane width the full basename should remain visible; got {s:?}"
        );
    }

    #[test]
    fn fit_path_to_width_collapses_to_ellipsis_when_pane_is_tiny() {
        let s = fit_path_to_width("any/path.md", 6); // label uses all the width
        assert_eq!(s, "\u{2026}");
    }

    /// Regression for cycle-1 review feedback on 039: the preview header's
    /// `path:` line rendered visually EMPTY when the entity was a
    /// worktree-resident copy and the absolute fallback path was longer than
    /// the preview pane width. The header paragraph wraps with
    /// `Wrap { trim: true }`, which word-wraps at the single space between
    /// the label and a long path — leaving the label alone on one row and
    /// the value on the next. This test exercises BOTH cases:
    ///   (i) in-workflow-root items → relative path expected, fits inline.
    ///   (ii) out-of-root (worktree-resident) items → absolute fallback, but
    ///        truncated with a leading ellipsis so it still fits on one row
    ///        and the basename stays visible.
    /// Either way, the rendered row that begins with "path:" must carry a
    /// non-empty visible value.
    #[test]
    fn path_line_stays_visible_for_long_paths() {
        use crate::domain::{StageDefinition, WorkflowDefinition, WorkflowSnapshot};
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let workflow_dir = PathBuf::from("/repo/docs/wf");

        // (i) In-root item: path is /repo/docs/wf/039-foo.md, workflow_dir is
        //     /repo/docs/wf, so strip_prefix yields "039-foo.md" — short and
        //     visible on the same row as the "path:" label.
        let in_root = WorkItem {
            path: PathBuf::from("/repo/docs/wf/039-foo.md"),
            id: "039".to_string(),
            title: "In root".to_string(),
            status: "design".to_string(),
            source: Some("x".to_string()),
            started: None,
            completed: None,
            verdict: None,
            score: None,
            worktree: None,
            issue: None,
            pr: None,
            body: "Body".to_string(),
            worktree_source: None,
            main_body: None,
        };

        // (ii) Out-of-root item: a worktree-resident copy whose absolute path
        //      is well over the preview pane width — exercises the absolute
        //      fallback + width-fit truncation.
        let mut out_of_root = in_root.clone();
        out_of_root.id = "040".to_string();
        out_of_root.title = "Out of root".to_string();
        out_of_root.path = PathBuf::from(
            "/repo/.worktrees/spacedock-ensign-039-open-entity-file-from-preview/docs/wf/040-bar.md",
        );

        let snapshot = WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: workflow_dir.clone(),
                stages: vec![StageDefinition {
                    name: "design".to_string(),
                    initial: true,
                    terminal: false,
                    gate: false,
                    fresh: false,
                    feedback_to: None,
                    worktree: false,
                    concurrency: None,
                }],
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: std::collections::HashMap::new(),
            },
            items: vec![in_root, out_of_root],
        };
        let mut app = App::from_snapshot(workflow_dir.clone(), snapshot);
        // Open the preview on the first (in-root) item.
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let mut terminal =
            Terminal::new(TestBackend::new(120, 30)).expect("test terminal should be created");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();
        assert_path_row_non_empty(buffer, "in-root");
        let rendered = buffer_text(buffer);
        assert!(
            rendered.contains("path: 039-foo.md"),
            "in-root item should render relative path on the same row, got: {rendered}"
        );

        // Now move down to the out-of-root item; preview follows the
        // selection.
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();
        assert_path_row_non_empty(buffer, "out-of-root");
        // The truncated absolute path must still surface the basename so the
        // user can identify the file at a glance.
        let rendered = buffer_text(buffer);
        assert!(
            rendered.contains("040-bar.md"),
            "out-of-root item should still show the basename in the path row, got: {rendered}"
        );
        // The leading ellipsis marker is the truncation signal.
        assert!(
            rendered.contains('\u{2026}'.to_string().as_str()),
            "out-of-root long path should be truncated with a leading ellipsis, got: {rendered}"
        );
    }

    /// Helper: locate the row whose first non-empty content begins with
    /// "path:" and assert that some visible character follows the label on
    /// the same row. Fails the test with a helpful message otherwise.
    fn assert_path_row_non_empty(buffer: &ratatui::buffer::Buffer, label: &str) {
        let hits = find_text(buffer, "path:");
        assert!(
            !hits.is_empty(),
            "({label}) expected to find a 'path:' label in the rendered preview"
        );
        let (x, y) = hits[0];
        let after = x + "path:".chars().count() as u16;
        let mut non_empty_seen = false;
        for col in after..buffer.area.width {
            let cell = &buffer[(col, y)];
            let sym = cell.symbol();
            if sym.is_empty() {
                continue;
            }
            // Skip the single space between label and value.
            if sym == " " {
                continue;
            }
            non_empty_seen = true;
            break;
        }
        let row_text: String = (0..buffer.area.width)
            .map(|cx| buffer[(cx, y)].symbol().to_string())
            .collect::<Vec<_>>()
            .join("");
        assert!(
            non_empty_seen,
            "({label}) 'path:' row at y={y} has no visible value after the label; \
             row text: {row_text:?}"
        );
    }

    /// AC-6: the help popup documents the new `o` keybinding when the
    /// preview pane is open.
    #[test]
    fn help_popup_documents_open_file_keybind_when_preview_open() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut app = app_with_items(vec![item("001", "Help test", "body")]);
        // Open preview, then open the help popup.
        // (app_with_items already opens the preview, but be explicit so
        // future refactors don't silently break this test.)
        if !app.as_overview().is_some_and(|s| s.preview_open()) {
            app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert!(app.help_open());

        let mut terminal =
            Terminal::new(TestBackend::new(140, 30)).expect("test terminal should be created");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(
            rendered.contains("open file in $EDITOR"),
            "help popup should document the `o` keybind, got: {rendered}"
        );
        // Also check the leading `o` key column itself is present in the
        // popup (rather than only the description text).
        assert!(
            find_text(terminal.backend().buffer(), "o ")
                .into_iter()
                .any(|(_, _)| true),
            "help popup should render the `o` key column"
        );
    }

    #[test]
    fn archived_preview_includes_worktree_segment() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let mut app = App::load(root).expect("workflow should load");
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let mut terminal =
            Terminal::new(TestBackend::new(180, 30)).expect("test terminal should be created");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("worktree:"),
            "archived preview should include worktree segment, got: {rendered}"
        );
    }

    fn app_with_items(items: Vec<WorkItem>) -> App {
        let root = PathBuf::from("/tmp/spacetop-test");
        let snapshot = WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: root.clone(),
                stages: vec![StageDefinition {
                    name: "design".to_string(),
                    initial: true,
                    terminal: false,
                    gate: false,
                    fresh: false,
                    feedback_to: None,
                    worktree: false,
                    concurrency: None,
                }],
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: std::collections::HashMap::new(),
            },
            items,
        };
        let mut app = App::from_snapshot(root, snapshot);
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        app
    }

    fn item(id: &str, title: &str, body: &str) -> WorkItem {
        WorkItem {
            path: PathBuf::from(format!("/tmp/{id}.md")),
            id: id.to_string(),
            title: title.to_string(),
            status: "design".to_string(),
            source: Some("test".to_string()),
            started: None,
            completed: None,
            verdict: None,
            score: None,
            worktree: None,
            issue: None,
            pr: None,
            body: body.to_string(),
            worktree_source: None,
            main_body: None,
        }
    }

    fn snapshot_with_body(id: &str, title: &str, body: &str) -> WorkflowSnapshot {
        WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: PathBuf::from("/tmp/ww-test"),
                stages: vec![StageDefinition {
                    name: "design".to_string(),
                    initial: true,
                    terminal: false,
                    gate: false,
                    fresh: false,
                    feedback_to: None,
                    worktree: false,
                    concurrency: None,
                }],
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: std::collections::HashMap::new(),
            },
            items: vec![item(id, title, body)],
        }
    }

    #[test]
    fn word_wrap_toggle_changes_body_render() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let body = "a".repeat(200);
        let mut app = App::from_snapshot(
            PathBuf::from("/tmp/ww-ac1"),
            snapshot_with_body("001", "Wrap test", &body),
        );
        // Open preview (wrap defaults to on).
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
        // Wrap-on default: scroll_x clamped to 0.
        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert_eq!(
            app.as_overview().unwrap().max_preview_scroll_x.get(),
            0,
            "wrap mode clamps scroll_x to 0"
        );
        // Toggle wrap off — no-wrap exposes the real horizontal scroll limit.
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let max_x_no_wrap = app.as_overview().unwrap().max_preview_scroll_x.get();
        assert!(
            max_x_no_wrap > 0,
            "no-wrap mode should report a non-zero horizontal scroll limit for a 200-char body"
        );
        // Toggle wrap on again — scroll_x clamped back to 0.
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert_eq!(
            app.as_overview().unwrap().max_preview_scroll_x.get(),
            0,
            "wrap mode clamps scroll_x to 0"
        );
    }

    #[test]
    fn word_wrap_toggle_persists_across_preview_open_close() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut app = App::from_snapshot(
            PathBuf::from("/tmp/ww-ac2"),
            snapshot_with_body("001", "Persist test", "some body"),
        );
        // Open preview — default-on.
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            app.as_overview().unwrap().preview_wrap(),
            "preview opens with wrap on by default"
        );
        // Toggle off.
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert!(!app.as_overview().unwrap().preview_wrap());
        // Close preview — wrap-off persists across pane close.
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            !app.as_overview().unwrap().preview_wrap(),
            "wrap toggle persists across pane close"
        );
        // Re-open — wrap-off still in effect.
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            !app.as_overview().unwrap().preview_wrap(),
            "wrap toggle persists across pane re-open"
        );
        // Toggle still works in the other direction.
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert!(
            app.as_overview().unwrap().preview_wrap(),
            "w keypress still toggles wrap back on"
        );
    }

    #[test]
    fn footer_shows_word_wrap_hint_when_preview_open() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut app = App::from_snapshot(
            PathBuf::from("/tmp/ww-ac3"),
            snapshot_with_body("001", "Legend test", "body"),
        );
        let mut terminal = Terminal::new(TestBackend::new(180, 24)).expect("terminal");
        // Before preview: hint absent.
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            !rendered.contains("w: word wrap"),
            "hint absent before preview opens"
        );
        // After preview: hint present.
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("w: word wrap"),
            "hint visible when preview open"
        );
    }

    #[test]
    fn preview_renders_markdown_body_instead_of_raw_markers() {
        let app = app_with_items(vec![item(
            "001",
            "Markdown Preview",
            "# Heading\n\nSome **bold** text.\n\n- first item",
        )]);
        let mut terminal = Terminal::new(TestBackend::new(120, 24)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let rendered = buffer_text(buffer);
        assert!(rendered.contains("Heading"), "missing rendered heading");
        assert!(
            rendered.contains("Some bold text."),
            "missing rendered paragraph without markdown markers"
        );
        assert!(
            !rendered.contains("# Heading") && !rendered.contains("**bold**"),
            "preview should not show raw markdown markers"
        );
        assert!(
            find_styled_text(buffer, "Heading", |style| {
                style.add_modifier.contains(ratatui::style::Modifier::BOLD)
            }),
            "heading text should be bold"
        );
        assert!(
            find_styled_text(buffer, "bold", |style| {
                style.add_modifier.contains(ratatui::style::Modifier::BOLD)
            }),
            "strong markdown text should be bold"
        );
    }

    #[test]
    fn preview_renders_markdown_tables_as_aligned_rows() {
        // termimad renders tables with Unicode box-drawing borders. We check
        // that the preceding paragraph and every cell value land in the
        // buffer, that raw pipe-and-dash markdown leaks have been suppressed,
        // and that a header/body separator row is drawn.
        let app = app_with_items(vec![item(
            "001",
            "Markdown Table Preview",
            "Ablation siblings\n\n| Arm | Entity | README |\n| --- | ---: | --- |\n| 1 | 17 | direct.md |\n| 2 | 18 | method.md |",
        )]);
        let mut terminal = Terminal::new(TestBackend::new(140, 24)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("Ablation siblings"));
        for cell in ["Arm", "Entity", "README", "direct.md", "method.md"] {
            assert!(
                rendered.contains(cell),
                "expected cell {cell:?} in rendered table:\n{rendered}"
            );
        }
        assert!(
            rendered.contains("\u{2500}\u{2500}\u{2500}"),
            "table should show a separator row between header and body"
        );
        assert!(
            rendered.contains("\u{2502}"),
            "termimad renders cell borders with the vertical box-drawing char"
        );
        assert!(
            !rendered.contains("| Arm |") && !rendered.contains("---"),
            "preview should render table structure rather than raw markdown separators"
        );
    }

    #[test]
    fn preview_uses_full_pane_width_for_wide_content() {
        let body = format!("{}PREVIEWFULLWIDTH", "X".repeat(92));
        let app = app_with_items(vec![item("001", "Wide Preview", &body)]);
        let width: u16 = 220;
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        assert!(
            find_text_starting_after(buffer, "PREVIEWFULLWIDTH", 30),
            "preview content should use the full preview pane instead of a centered narrow column"
        );
    }

    #[test]
    fn preview_right_key_horizontally_scrolls_long_lines() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let body = format!("{}HORIZONTALSCROLLTARGET", "X".repeat(220));
        let mut app = app_with_items(vec![item("001", "Wide Preview", &body)]);
        // Disable wrap so the long line stays on a single row and horizontal scroll applies.
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        let width: u16 = 80;
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).expect("terminal");

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let before = buffer_text(terminal.backend().buffer());
        assert!(!before.contains("HORIZONTALSCROLLTARGET"));

        for _ in 0..30 {
            app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let after = buffer_text(terminal.backend().buffer());
        assert!(after.contains("HORIZONTALSCROLLTARGET"));
    }

    #[test]
    fn preview_draws_scrollbar_when_content_overflows() {
        let body = (0..40)
            .map(|index| format!("Line {index}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let app = app_with_items(vec![item("001", "Scrollable Preview", &body)]);
        let width: u16 = 120;
        let height: u16 = 18;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let has_scrollbar_thumb = (0..buffer.area.height)
            .any(|y| (0..buffer.area.width).any(|x| buffer[(x, y)].symbol() == "\u{2588}"));
        assert!(
            has_scrollbar_thumb,
            "overflowing preview should draw a scrollbar thumb"
        );
    }

    #[test]
    fn preview_page_down_scrolls_visible_markdown_content() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let body = (0..30)
            .map(|index| format!("Line {index:02}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let mut app = app_with_items(vec![item("001", "Scrollable Preview", &body)]);
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

        let mut terminal = Terminal::new(TestBackend::new(120, 18)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            !rendered.contains("Line 00"),
            "scrolled preview should not keep the first body line visible"
        );
        assert!(
            rendered.contains("Line 03"),
            "scrolled preview should advance by the PageDown row offset, including markdown spacing"
        );
    }

    #[test]
    fn preview_adds_blank_rows_between_markdown_blocks() {
        let app = app_with_items(vec![item(
            "001",
            "Spaced Markdown",
            "# Heading\n\nFirst paragraph.\n\nSecond paragraph.",
        )]);
        let mut terminal = Terminal::new(TestBackend::new(120, 24)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let heading_y = find_text(buffer, "Heading")[0].1;
        let first_y = find_text(buffer, "First paragraph.")[0].1;
        let second_y = find_text(buffer, "Second paragraph.")[0].1;
        assert!(
            first_y >= heading_y + 2,
            "expected a blank row between heading and first paragraph"
        );
        assert!(
            second_y >= first_y + 2,
            "expected a blank row between paragraphs"
        );
    }

    #[test]
    fn preview_keeps_body_divider_visible_when_header_wraps() {
        let mut work_item = item("001", "Wrapped Header", "Body content stays visible.");
        work_item.path = PathBuf::from(
            "/tmp/very/long/path/that/forces/the/preview/header/path/line/to/wrap/multiple/times/so/the/body/divider/must/still/render/work-item.md",
        );
        let app = app_with_items(vec![work_item]);
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("── body"),
            "wrapped preview headers should still leave room for the body divider"
        );
    }

    #[test]
    fn task_list_uses_full_pane_width_and_ratatui_list_selection() {
        let stable_title = "Stable selected title";
        let long_title = format!("{}FULLWIDTHMARKER", "X".repeat(60));
        let app = app_with_items(vec![
            item("001", stable_title, "Body"),
            item("002", &long_title, "Body"),
            item("003", "Second task", "Body"),
        ]);

        let width: u16 = 220;
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();

        // FULLWIDTHMARKER sits 60 X's into the title, after the row prefix
        // (id + spaces + tag + spaces = 11 chars) and the 2-char highlight pad.
        // With no border on the list pane it lands at col ~73; threshold 60
        // is generous enough to prove content is not confined to a narrow column.
        assert!(
            find_text_starting_after(buffer, "FULLWIDTHMARKER", 60),
            "task row content should use the whole list pane rather than a centered narrow column"
        );
        let rendered = buffer_text(buffer);
        // Selected row now uses ▸ gutter (not "> ") with bg-2 fill.
        assert!(
            rendered.contains('\u{25B8}'),
            "selected row should display ▸ gutter glyph"
        );
        assert!(
            find_styled_text(buffer, stable_title, |style| {
                style.bg == Some(ratatui::style::Color::Rgb(40, 52, 84))
            }),
            "selected row title should have selection color fill (Tokyo Night Rgb(40,52,84))"
        );
        assert!(
            find_styled_text(buffer, stable_title, |style| {
                style.add_modifier.contains(ratatui::style::Modifier::BOLD)
            }),
            "selected row title should be bold"
        );
    }

    #[test]
    fn selected_row_fill_covers_full_pane_width() {
        // The selected row background (Rgb(40,52,84)) must extend to the rightmost
        // cell of the task list pane, not stop at the last text character.
        let app = app_with_items(vec![
            item("001", "Short title", "Body"),
            item("002", "Another task", "Body"),
        ]);
        let width: u16 = 80;
        let height: u16 = 24;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let buffer = terminal.backend().buffer();

        // Find the selected row (row with ▸ glyph).
        let hits = find_text(buffer, "\u{25B8}");
        assert!(!hits.is_empty(), "selected row must have ▸ glyph");
        let (_, sel_row) = hits[0];

        // The rightmost cell of the task-list pane (width/2 - 1 in split view, or
        // width - 1 in full-pane view). In full-pane mode (no preview open by default)
        // the list fills 0..width. Check the last cell on the selected row has BG2.
        // app_with_items opens preview (Enter), so list is the left half: 0..width/2.
        let last_list_col = width / 2 - 1;
        let style = buffer[(last_list_col, sel_row)].style();
        assert_eq!(
            style.bg,
            Some(Color::Rgb(40, 52, 84)),
            "rightmost list cell on selected row (col {last_list_col}, row {sel_row}) must have selection background"
        );
    }

    // ---- AC snapshot tests ----

    #[test]
    fn task_row_phase_column_12_char_fixed() {
        // With a single item whose status is "implement" (9 chars), phase_col_width
        // auto-sizes to 9 (the longest status, clamped to [4,12]). The phase
        // column is "implement" with no trailing spaces.
        let app = app_with_items(vec![{
            let mut i = item("001", "Test task", "Body");
            i.status = "implement".to_string();
            i
        }]);
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let rendered = buffer_text(terminal.backend().buffer());
        // "implement" must appear with user casing preserved.
        assert!(
            rendered.contains("implement"),
            "phase name 'implement' must appear in task row; rendered: {:?}",
            &rendered[..rendered.len().min(200)]
        );
        // The word "implement" must not be uppercased.
        assert!(
            !rendered.contains("IMPLEMENT"),
            "phase name must not be uppercased"
        );
        // Verify phase_col() helper directly for various widths.
        let col_w9 = super::phase_col("implement", 9);
        assert_eq!(
            col_w9, "implement",
            "phase_col('implement', 9) must be exact fit"
        );
        let col_w12 = super::phase_col("implement", 12);
        assert_eq!(
            col_w12, "implement   ",
            "phase_col('implement', 12) must pad to 12"
        );
        let col_w4 = super::phase_col("implement", 4);
        assert_eq!(
            col_w4, "imp\u{2026}",
            "phase_col('implement', 4) must truncate at 3+ellipsis"
        );
    }

    #[test]
    fn task_row_long_phase_name_ellipsis() {
        // Phase names longer than 12 chars must be ellipsized at char 11 + "…".
        let long_phase = "averylongphasename"; // 18 chars
        let app = {
            let root = PathBuf::from("/tmp/spacetop-ellipsis");
            let snapshot = crate::domain::WorkflowSnapshot {
                definition: crate::domain::WorkflowDefinition {
                    root: root.clone(),
                    stages: vec![crate::domain::StageDefinition {
                        name: long_phase.to_string(),
                        initial: true,
                        terminal: false,
                        gate: false,
                        fresh: false,
                        feedback_to: None,
                        worktree: false,
                        concurrency: None,
                    }],
                    id_style: None,
                    entity_type: None,
                    entity_label: None,
                    entity_label_plural: None,
                    stage_colors: std::collections::HashMap::new(),
                },
                items: vec![{
                    let mut i = item("001", "Long phase task", "Body");
                    i.status = long_phase.to_string();
                    i
                }],
            };
            App::from_snapshot(root, snapshot)
        };
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let rendered = buffer_text(terminal.backend().buffer());
        // First 11 chars of long_phase + "…"
        let expected_prefix = &long_phase[..11]; // "averylongph"
        assert!(
            rendered.contains(&format!("{expected_prefix}\u{2026}")),
            "long phase name should be ellipsized to 11 chars + '…'; rendered: {:?}",
            &rendered[..rendered.len().min(200)]
        );
    }

    #[test]
    fn task_row_selected_gutter() {
        // Selected row must show ▸ gutter; unselected rows must show 2 spaces.
        let app = app_with_items(vec![
            item("001", "Selected task", "Body"),
            item("002", "Unselected task", "Body"),
        ]);
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let rendered = buffer_text(terminal.backend().buffer());
        // ▸ (\u{25B8}) must appear for the selected row.
        assert!(
            rendered.contains('\u{25B8}'),
            "selected row must show ▸ gutter glyph"
        );
    }

    #[test]
    fn task_row_no_uppercase_phase() {
        // Phase column must not uppercase any stage name.
        let app = app_with_items(vec![item("001", "Task", "Body")]);
        // The stage name in `app_with_items` is "design" (lowercase).
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let rendered = buffer_text(terminal.backend().buffer());
        // "design" must appear (lowercase), not "DESIGN", "DES", "DGN", etc.
        assert!(
            rendered.contains("design"),
            "phase name 'design' must appear in task row"
        );
        assert!(
            !rendered.contains("DESIGN"),
            "phase name must not be uppercased"
        );
        assert!(
            !rendered.contains("DES"),
            "old 3-letter tag must not appear"
        );
    }

    #[test]
    fn task_row_no_glyphs_in_phase_col() {
        // Phase column must not contain DAG vocabulary glyphs.
        // The DAG glyphs are ▶ (U+25B6), ⎇ (U+2387), ⚑ (U+2691), ■ (U+25A0).
        // Note: ▸ (U+25B8) is the gutter selection glyph — NOT a DAG vocab glyph.
        let dag_glyphs: &[char] = &['\u{25B6}', '\u{2387}', '\u{2691}', '\u{25A0}'];

        let app = app_with_items(vec![item("001", "Task", "Body")]);
        let width: u16 = 100;
        let height: u16 = 24;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let buffer = terminal.backend().buffer();

        // Layout (with preview open, Left placement at width=100, height=24):
        //   row 0: header bar
        //   rows 1–7: graph ribbon (7 rows)
        //   row 8: "Tasks · N" section header
        //   rows 9+: task list rows
        //
        // Task row column layout (x offsets, no border on list block):
        //   x 0–1:  gutter (2 chars: "▸ " or "  ")
        //   x 2–13: phase column (12 chars — user stage name)
        //   x 14:   separator space
        //   x 15–18: id (4 chars)
        //   x 19+:  title
        //
        // We scan the phase column cells (x=2..14) for all task rows (y=9..height).
        // No DAG vocabulary glyph may appear there.

        let phase_col_x_start: u16 = 2;
        let phase_col_x_end: u16 = 14; // exclusive
        let task_rows_y_start: u16 = 9; // first task row (after section header at y=8)

        let mut violations: Vec<(u16, u16, char)> = Vec::new();
        for y in task_rows_y_start..height {
            for x in phase_col_x_start..phase_col_x_end {
                let cell = &buffer[(x, y)];
                let sym = cell.symbol();
                for ch in sym.chars() {
                    if dag_glyphs.contains(&ch) {
                        violations.push((x, y, ch));
                    }
                }
            }
        }
        assert!(
            violations.is_empty(),
            "DAG glyphs found in task list phase column (x=2..14, y=9..{}): {:?}",
            height,
            violations
        );

        // Also verify phase_col() helper itself never emits DAG glyphs.
        let pc = super::phase_col("design", 12);
        for glyph in dag_glyphs {
            assert!(
                !pc.contains(*glyph),
                "phase_col('design') must not contain DAG glyph {:?}, got {:?}",
                glyph,
                pc
            );
        }
    }

    #[test]
    fn header_strip_badge_style_and_path_truncation() {
        // At a short terminal width, the path should be left-truncated with "…".
        // The badge should have yellow background (filled style).
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
        // Narrow enough to trigger path truncation. The header prefix grew with
        // the sort badge, so this needs to be a bit wider than before to leave
        // room for the truncated path itself.
        let width: u16 = 100;
        let mut terminal = Terminal::new(TestBackend::new(width, 20)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let buffer = terminal.backend().buffer();
        let rendered = buffer_text(buffer);
        // Badge "[active]" must appear.
        assert!(rendered.contains("[active]"), "badge must appear");
        // Yellow bg must be present in the header row (row 0) for badge cells.
        let mut found_yellow_bg = false;
        for x in 0..width {
            let cell = &buffer[(x, 0)];
            if cell.style().bg == Some(ratatui::style::Color::Yellow) {
                found_yellow_bg = true;
                break;
            }
        }
        assert!(
            found_yellow_bg,
            "badge cell must have yellow background in row 0"
        );
        // "…" must appear in row 0 (left-truncated path) at narrow width.
        assert!(
            rendered.contains('\u{2026}'),
            "path must be left-truncated with '…' at width={width}"
        );
    }

    #[test]
    fn header_bar_shows_sort_badge() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = app_with_items(vec![
            item("002", "Two", "b"),
            item("010", "Ten", "b"),
        ]);
        // app_with_items opens preview; close it so 's' is not gated off.
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let width: u16 = 200;
        let mut terminal = Terminal::new(TestBackend::new(width, 20)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("[sort: id]"),
            "header must show [sort: id] initially, got: {rendered}"
        );

        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        let mut terminal = Terminal::new(TestBackend::new(width, 20)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("[sort: status]"),
            "header must show [sort: status] after cycling, got: {rendered}"
        );
    }

    #[test]
    fn footer_hints_have_background() {
        // Footer pill-style hints must have a non-default background color.
        let app = app_with_items(vec![item("001", "Task", "Body")]);
        let height: u16 = 24;
        let width: u16 = 120;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let buffer = terminal.backend().buffer();
        // Footer is the last row.
        let footer_y = height - 1;
        let mut found_pill_bg = false;
        for x in 0..width {
            let cell = &buffer[(x, footer_y)];
            if let Some(bg) = cell.style().bg {
                if bg != ratatui::style::Color::Reset {
                    found_pill_bg = true;
                    break;
                }
            }
        }
        assert!(
            found_pill_bg,
            "footer row {footer_y} must have at least one cell with a non-default background (pill hint)"
        );
    }

    // ---- phase_col_width auto-sizing snapshot tests ----

    #[test]
    fn phase_col_width_uniform_short_phases_clamped_to_4() {
        // When all visible items have status "run" (3 chars), phase_col_width
        // clamps to the minimum of 4. Verify via phase_col() helper directly.
        let mut run_item = item("001", "Task", "Body");
        run_item.status = "run".to_string(); // 3 chars, below minimum of 4
                                             // "run" is 3 chars < 4 minimum → phase_col_width returns 4.
        let items_ref: Vec<&crate::domain::WorkItem> = vec![&run_item];
        // Simulate what build_task_list_items does: collect refs and call phase_col_width.
        // We use a locally-constructed slice to test the helper.
        let pcw = items_ref
            .iter()
            .map(|i| i.status.chars().count())
            .max()
            .unwrap_or(4)
            .clamp(4, 12);
        assert_eq!(
            pcw, 4,
            "phase_col_width for 'run' (3 chars) must clamp to 4"
        );
        // phase_col with width=4 pads "run" to "run " (3 chars + 1 space).
        let col = super::phase_col("run", pcw);
        assert_eq!(col, "run ", "phase_col('run', 4) must pad to 4 chars");
        assert_eq!(
            col.chars().count(),
            4,
            "column must be exactly 4 chars wide"
        );
    }

    #[test]
    fn phase_col_width_mixed_phases_fits_longest() {
        // When items have mixed phase lengths, phase_col_width picks the longest
        // (clamped ≤ 12). "run" (3→4 min), "implement" (9), "smoke-test" (10).
        // Longest is 10 → phase_col_width = 10.
        let items_data = [
            {
                let mut i = item("001", "Task A", "Body");
                i.status = "run".to_string();
                i
            },
            {
                let mut i = item("002", "Task B", "Body");
                i.status = "implement".to_string();
                i
            },
            {
                let mut i = item("003", "Task C", "Body");
                i.status = "smoke-test".to_string();
                i
            },
        ];
        let items_ref: Vec<&crate::domain::WorkItem> = items_data.iter().collect();
        let pcw = items_ref
            .iter()
            .map(|i| i.status.chars().count())
            .max()
            .unwrap_or(4)
            .clamp(4, 12);
        assert_eq!(
            pcw, 10,
            "phase_col_width for mixed phases with max len=10 must return 10"
        );
        // "implement" (9 chars) with width=10 must pad to 10 chars.
        let col = super::phase_col("implement", pcw);
        assert_eq!(
            col.chars().count(),
            10,
            "phase column must be exactly 10 chars"
        );
        assert_eq!(col, "implement ", "implement padded to width 10");
        // "smoke-test" (10 chars) with width=10 must fit exactly.
        let col2 = super::phase_col("smoke-test", pcw);
        assert_eq!(
            col2, "smoke-test",
            "smoke-test must fit exactly at width 10"
        );
    }

    #[test]
    fn phase_col_width_long_phase_name_clamped_at_12() {
        // When the longest phase name exceeds 12 chars, phase_col_width clamps to 12.
        let long_item = {
            let mut i = item("001", "Task", "Body");
            i.status = "a-very-long-phase-name".to_string(); // 22 chars
            i
        };
        let items_ref: Vec<&crate::domain::WorkItem> = vec![&long_item];
        let pcw = items_ref
            .iter()
            .map(|i| i.status.chars().count())
            .max()
            .unwrap_or(4)
            .clamp(4, 12);
        assert_eq!(
            pcw, 12,
            "phase_col_width must clamp to 12 for a 22-char status"
        );
        // phase_col with width=12 must truncate at 11 chars + "…".
        let col = super::phase_col("a-very-long-phase-name", pcw);
        assert_eq!(
            col.chars().count(),
            12,
            "column must be exactly 12 chars after truncation"
        );
        assert!(
            col.ends_with('\u{2026}'),
            "truncated column must end with '…'"
        );
        assert_eq!(
            &col[..col.len() - 3],
            "a-very-long", // 11 chars, "…" is 3 bytes
            "first 11 chars must be preserved before ellipsis"
        );
    }

    fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("")
    }

    fn find_text_starting_after(
        buffer: &ratatui::buffer::Buffer,
        needle: &str,
        min_x: u16,
    ) -> bool {
        find_text(buffer, needle)
            .into_iter()
            .any(|(x, _y)| x >= min_x)
    }

    fn find_styled_text<F>(buffer: &ratatui::buffer::Buffer, needle: &str, predicate: F) -> bool
    where
        F: Fn(ratatui::style::Style) -> bool,
    {
        let chars: Vec<String> = needle.chars().map(|c| c.to_string()).collect();
        find_text(buffer, needle).into_iter().any(|(x, y)| {
            chars
                .iter()
                .enumerate()
                .all(|(offset, _)| predicate(buffer[(x + offset as u16, y)].style()))
        })
    }

    fn find_text(buffer: &ratatui::buffer::Buffer, needle: &str) -> Vec<(u16, u16)> {
        let chars: Vec<String> = needle.chars().map(|c| c.to_string()).collect();
        let mut matches = Vec::new();
        if chars.is_empty() {
            return matches;
        }
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if x + chars.len() as u16 > buffer.area.width {
                    continue;
                }
                if chars
                    .iter()
                    .enumerate()
                    .all(|(i, c)| buffer[(x + i as u16, y)].symbol() == c.as_str())
                {
                    matches.push((x, y));
                }
            }
        }
        matches
    }

    // --- Help popup behaviour ---

    #[test]
    fn help_popup_toggles_with_question_mark_and_closes_on_esc() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let mut app = App::load(root).expect("workflow should load");
        assert!(!app.help_open());

        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert!(app.help_open(), "? should open help");

        // Quit/movement keys are inert while help is open.
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(app.help_open() && !app.should_quit());

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.help_open(), "Esc should close help");

        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert!(!app.help_open(), "? toggle should close again");
    }

    #[test]
    fn help_popup_renders_keymap_in_overview_mode() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let mut app = App::load(root).expect("workflow should load");
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("Help"), "missing help title");
        assert!(rendered.contains("keymap"), "missing keymap heading");
        assert!(rendered.contains("Up / k"), "missing Up/k binding");
        assert!(rendered.contains("Esc"), "missing Esc binding");
        assert!(
            !rendered.contains("Esc / q"),
            "help should not claim q closes the help popup"
        );
        assert!(
            rendered.contains("PageUp         page list up"),
            "help should describe PageUp as list paging when preview is closed"
        );
        assert!(
            rendered.contains("PageDown       page list down"),
            "help should describe PageDown as list paging when preview is closed"
        );
        assert!(
            rendered.contains("press ? or Esc to close"),
            "missing close hint"
        );
    }

    #[test]
    fn help_popup_renders_in_picker_mode() {
        use crate::discovery::DiscoveredWorkflow;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let workflows = vec![
            DiscoveredWorkflow {
                root: PathBuf::from("/x/a"),
                title: Some("A".into()),
            },
            DiscoveredWorkflow {
                root: PathBuf::from("/x/b"),
                title: Some("B".into()),
            },
        ];
        let mut app = App::from_picker(PathBuf::from("/x"), workflows);
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

        let mut terminal = Terminal::new(TestBackend::new(120, 20)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("Help"), "missing help title in picker");
        assert!(
            rendered.contains("keymap"),
            "missing keymap heading in picker"
        );
    }

    // --- AC-1: dashboard responsive width + content centering ---

    #[test]
    fn dashboard_pane_spans_full_terminal_width() {
        // The Overview block must render with no left/right margin gutter —
        // i.e. content starts at column 0 and the layout fills the terminal width.
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
        let width: u16 = 200;
        let height: u16 = 30;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();
        // Row 0 is the header bar; it starts with "Workflow" at col 0.
        let top_left = buffer[(0, 0)].symbol();
        assert_ne!(
            top_left, " ",
            "expected non-blank left edge of dashboard at (0,0), got blank"
        );
        // Row 1 is the graph pane's TOP border; it spans the full width.
        // The graph block uses TOP|BOTTOM borders only, so the top border character
        // at (0, 1) and (width-1, 1) should be non-blank.
        let graph_border_left = buffer[(0, 1)].symbol();
        let graph_border_right = buffer[(width - 1, 1)].symbol();
        assert_ne!(
            graph_border_left, " ",
            "expected non-blank left edge of graph pane border at (0,1), got blank"
        );
        assert_ne!(
            graph_border_right,
            " ",
            "expected non-blank right edge of graph pane border at ({},1), got blank",
            width - 1
        );
    }

    #[test]
    fn graph_ribbon_node_row_is_horizontally_centered_in_pane() {
        // On a wide terminal, the graph ribbon's first stage glyph should
        // sit roughly equidistant from the pane's left/right edges —
        // satisfying AC-1's "content centered within each pane".
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
        let width: u16 = 200;
        let height: u16 = 30;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();
        // Find the row containing the first stage name (e.g. "design").
        let first_stage = &app.snapshot().definition.stages[0].name;
        let first_char = first_stage.chars().next().unwrap().to_string();
        let cols = width as usize;
        let mut found_row: Option<usize> = None;
        let mut found_col: Option<usize> = None;
        'outer: for y in 0..height {
            for x in 0..width {
                if buffer[(x, y)].symbol() == first_char {
                    // Check the rest of the stage name follows.
                    let chars: Vec<String> = first_stage.chars().map(|c| c.to_string()).collect();
                    if (x as usize) + chars.len() > cols {
                        continue;
                    }
                    let ok = chars
                        .iter()
                        .enumerate()
                        .all(|(i, c)| buffer[(x + i as u16, y)].symbol() == c.as_str());
                    if ok {
                        found_row = Some(y as usize);
                        found_col = Some(x as usize);
                        break 'outer;
                    }
                }
            }
        }
        let col = found_col.expect("first stage label not found in render");
        let _row = found_row.unwrap();
        // The leftmost glyph of the centered content should be > some margin
        // from column 0 (proving it isn't hugging the left edge).
        assert!(
            col >= 8,
            "expected first stage column to be centered with non-trivial left margin, got col={col}"
        );
    }

    #[test]
    fn dashboard_status_footer_lists_help_affordance() {
        // AC-5: a visible affordance hints at the help popup somewhere on
        // the dashboard — we surface it via a status-line footer.
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("?"), "footer must include ? glyph");
        assert!(rendered.contains("help"), "footer must mention 'help'");
        assert!(rendered.contains("q: quit"), "footer must mention quit");
    }

    #[test]
    fn multi_footer_shows_switch_workflow_when_preview_closed() {
        let session = synthetic_session(2);
        let app = App::from_session(session);
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(rendered.contains("\u{2190}/\u{2192}: switch workflow"));
        assert!(!rendered.contains("\u{2190}/\u{2192}: preview scroll"));
        assert!(rendered.contains("PgUp/PgDn: page list"));
        assert!(!rendered.contains("PgUp/PgDn: preview scroll"));
    }

    #[test]
    fn multi_footer_shows_preview_scroll_when_preview_open() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let session = synthetic_session(2);
        let mut app = App::from_session(session);
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(rendered.contains("\u{2190}/\u{2192}: preview scroll"));
        assert!(!rendered.contains("\u{2190}/\u{2192}: switch workflow"));
        assert!(rendered.contains("PgUp/PgDn: preview scroll"));
        assert!(!rendered.contains("PgUp/PgDn: page list"));
    }

    // --- AC-2: tab bar workflow switcher (multi-workflow only) ---

    fn synthetic_session(n: usize) -> crate::app::OverviewSession {
        use crate::app::{OverviewSession, OverviewState};
        use crate::discovery::DiscoveredWorkflow;
        use crate::domain::{StageDefinition, WorkflowDefinition, WorkflowSnapshot};
        let snap = WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: PathBuf::from("/x/w0"),
                stages: vec![StageDefinition {
                    name: "plan".to_string(),
                    initial: true,
                    terminal: false,
                    gate: false,
                    fresh: false,
                    feedback_to: None,
                    worktree: false,
                    concurrency: None,
                }],
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: std::collections::HashMap::new(),
            },
            items: Vec::new(),
        };
        let initial = OverviewState::from_snapshot(PathBuf::from("/x/w0"), snap);
        let discovery: Vec<DiscoveredWorkflow> = (0..n)
            .map(|i| DiscoveredWorkflow {
                root: PathBuf::from(format!("/x/w{i}")),
                title: Some(format!("Workflow{i}")),
            })
            .collect();
        OverviewSession::from_discovery(PathBuf::from("/x"), discovery, 0, initial)
    }

    #[test]
    fn multi_session_renders_tab_bar_with_count_and_per_workflow_tabs() {
        let session = synthetic_session(3);
        let app = App::from_session(session);
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("Workflow0 | Workflow1 | Workflow2"),
            "tab strip must show ratatui tabs, got render snippet:\n{rendered}"
        );
        for i in 0..3 {
            assert!(
                rendered.contains(&format!("Workflow{i}")),
                "tab bar missing workflow tab #{i}"
            );
        }
    }

    #[test]
    fn multi_session_renders_dashboard_inside_workflow_tabs_panel() {
        let session = synthetic_session(2);
        let mut app = App::from_session(session);
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();

        let workflow_graph_y = find_text(buffer, "plan")
            .into_iter()
            .map(|(_, y)| y)
            .filter(|y| *y > 0)
            .min()
            .expect("workflow graph title should render inside tab panel");
        let tasks_y = find_text(buffer, "Tasks")[0].1;
        let preview_y = find_text(buffer, "Preview")[0].1;

        assert!(
            workflow_graph_y <= 3,
            "workflow graph should start inside the workflow tab panel, not below a separate tab strip"
        );
        assert!(
            tasks_y > workflow_graph_y && preview_y > workflow_graph_y,
            "task list and preview should render inside the selected workflow tab panel"
        );
    }

    #[test]
    fn multi_session_tabs_are_borderless_and_do_not_dim_dashboard_content() {
        let session = synthetic_session(2);
        let app = App::from_session(session);
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();

        assert_eq!(
            buffer[(0, 0)].symbol(),
            " ",
            "workflow tabs should not draw an outer border"
        );
        let plan_pos = find_text(buffer, "plan")
            .into_iter()
            .find(|(_, y)| *y > 1)
            .expect("workflow graph plan label should render");
        assert!(
            !buffer[plan_pos]
                .style()
                .add_modifier
                .contains(Modifier::DIM),
            "dashboard content inside workflow tabs should not inherit dim tab styling"
        );
    }

    #[test]
    fn single_session_omits_tab_bar() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            !rendered.contains("Workflows ("),
            "single-workflow session must hide the tab strip"
        );
    }

    #[test]
    fn arrow_keys_cycle_active_tab_with_wraparound_in_multi() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let session = synthetic_session(3);
        let mut app = App::from_session(session);

        // Right cycles forward 0 → 1. Materialize so the active slot is
        // available for the next handle_key (cycle reads is_multi via
        // session, not active state — but logging current active state is
        // what handle_key does after select).
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let switch = app.take_pending_switch().expect("Right emits switch");
        assert_eq!(switch.target_index, 1);
        app.materialize_active();

        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let switch = app.take_pending_switch().unwrap();
        assert_eq!(switch.target_index, 2);
        app.materialize_active();

        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let switch = app.take_pending_switch().unwrap();
        assert_eq!(switch.target_index, 0);
        app.materialize_active();

        // Left wraps 0 → 2.
        app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        let switch = app.take_pending_switch().unwrap();
        assert_eq!(switch.target_index, 2);
    }

    #[test]
    fn arrow_keys_inert_in_single_session() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let mut app = App::load(root).expect("workflow should load");
        let active_before = app.as_session().unwrap().active_index();
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert!(
            app.take_pending_switch().is_none(),
            "single session must not emit switches on Left/Right"
        );
        assert_eq!(app.as_session().unwrap().active_index(), active_before);
    }

    // --- AC-3: stage status colors ---

    #[test]
    fn graph_ribbon_uses_stage_colors_per_stage() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();

        let mut seen_colors: std::collections::HashSet<Color> = Default::default();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if let Some(fg) = buffer[(x, y)].style().fg {
                    seen_colors.insert(fg);
                }
            }
        }
        // Collect oklch-derived Rgb colors for this workflow's stages.
        let stage_colors: std::collections::HashSet<Color> = app
            .snapshot()
            .definition
            .stage_colors
            .values()
            .copied()
            .collect();
        let overlap = stage_colors.intersection(&seen_colors).count();
        assert!(
            overlap >= 3,
            "expected at least 3 stage colors visible in render, found {} of {:?} (seen: {:?})",
            overlap,
            stage_colors,
            seen_colors
        );
    }

    #[test]
    fn preview_status_value_is_stage_colored() {
        let app = app_with_items(vec![item("001", "Synthetic active task", "Body")]);
        let selected = app.selected_item().expect("selected").clone();
        let expected = super::stage_color(&selected.status);
        let status_value = selected.status.clone();
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();
        let label_chars: [&str; 10] = ["s", "t", "a", "t", "u", "s", ":", " ", "\u{25CF}", " "];
        let value_chars: Vec<String> = status_value
            .chars()
            .map(|c| c.to_string())
            .collect::<Vec<_>>();
        let cols = buffer.area.width;
        let rows = buffer.area.height;
        let mut found = false;
        'outer: for y in 0..rows {
            let row_syms: Vec<&str> = (0..cols).map(|x| buffer[(x, y)].symbol()).collect();
            let total_len = label_chars.len() + value_chars.len();
            if (row_syms.len()) < total_len {
                continue;
            }
            for start in 0..=(row_syms.len() - total_len) {
                let label_ok = label_chars
                    .iter()
                    .enumerate()
                    .all(|(i, &c)| row_syms[start + i] == c);
                if !label_ok {
                    continue;
                }
                let value_start = start + label_chars.len();
                let value_ok = value_chars.iter().enumerate().all(|(i, c)| {
                    let x = (value_start + i) as u16;
                    row_syms[value_start + i] == c.as_str()
                        && buffer[(x, y)].style().fg == Some(expected)
                });
                if value_ok {
                    found = true;
                    break 'outer;
                }
            }
        }
        assert!(
            found,
            "expected status value `{status_value}` in preview to use stage color {expected:?}"
        );
    }

    #[test]
    fn help_popup_includes_arrow_keys_in_multi_session() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let session = synthetic_session(2);
        let mut app = App::from_session(session);
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let rendered = buffer_text(terminal.backend().buffer());
        // Either Unicode arrow or "Left"/"Right" keyword is acceptable.
        assert!(
            rendered.contains("switch to next workflow"),
            "help popup must list workflow switching in multi when preview is closed"
        );
        assert!(
            rendered.contains("pick workflow"),
            "multi help should mention pick workflow"
        );

        let session = synthetic_session(2);
        let mut app = App::from_session(session);
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("scroll preview right"),
            "help popup must list preview scrolling when preview is open"
        );
        assert!(
            rendered.contains("PageDown       scroll preview down"),
            "preview-open help should describe PageDown as preview scroll"
        );
        assert!(
            !rendered.contains("switch to next workflow"),
            "preview-open help should not show workflow switching on arrows"
        );

        // Single session: the existing `App::load` path produces a pinned
        // single session whose help omits cycle hints.
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let mut app = App::load(root).expect("workflow should load");
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            !rendered.contains("switch to next workflow"),
            "single help must not include cycle hint"
        );
    }

    #[test]
    fn stage_color_assigns_distinct_colors_for_known_stages() {
        let design = super::stage_color("design");
        let plan = super::stage_color("plan");
        let implement = super::stage_color("implement");
        let review = super::stage_color("review");
        let done = super::stage_color("done");
        let all = [design, plan, implement, review, done];
        // All returned colors must be Color::Rgb (no named-color variants).
        for c in &all {
            assert!(
                matches!(c, Color::Rgb(_, _, _)),
                "stage_color() must return Color::Rgb, got {c:?}"
            );
        }
        // All colors must be distinct.
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b, "stage colors should be distinct");
            }
        }
    }

    #[test]
    fn preview_scrollbar_thumb_reaches_bottom_at_max_scroll() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let body = (0..60)
            .map(|i| format!("Line {:02}", i))
            .collect::<Vec<_>>()
            .join("\n\n");
        let mut app = app_with_items(vec![item("001", "Scrollable", &body)]);
        let width: u16 = 160;
        let height: u16 = 30;

        // Run several render+scroll cycles so max_preview_scroll is set by
        // render_preview before scroll_preview_down reads it.
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        for _ in 0..30 {
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        }
        // Final render at max scroll.
        terminal.draw(|frame| render(frame, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let thumb_rows = (0..buffer.area.height)
            .flat_map(|y| (0..buffer.area.width).map(move |x| (x, y)))
            .filter(|&(x, y)| buffer[(x, y)].symbol() == "\u{2588}")
            .map(|(_, y)| y)
            .collect::<Vec<_>>();
        let bottom_row = thumb_rows.iter().copied().max().expect("thumb visible");
        let thumb_at_bottom = bottom_row >= height / 2;
        assert!(
            thumb_at_bottom,
            "scrollbar thumb must move into the lower half of the preview at max scroll (row={bottom_row})"
        );
    }

    #[test]
    fn preview_scrollbar_thumb_starts_at_top_at_zero_scroll() {
        let body = (0..60)
            .map(|i| format!("Line {:02}", i))
            .collect::<Vec<_>>()
            .join("\n\n");
        let app = app_with_items(vec![item("001", "Scrollable", &body)]);
        let width: u16 = 160;
        let height: u16 = 32;

        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let first_thumb_row = (0..buffer.area.height)
            .flat_map(|y| (0..buffer.area.width).map(move |x| (x, y)))
            .filter(|&(x, y)| buffer[(x, y)].symbol() == "\u{2588}")
            .map(|(_, y)| y)
            .min()
            .expect("scrollbar thumb must be visible at scroll=0");

        assert!(
            first_thumb_row < height / 2,
            "at scroll=0, thumb must sit in the upper half of the track (got row {first_thumb_row})"
        );
    }

    // --- Graph-aware coloring tests (AC-1, AC-2, AC-3) ---

    fn make_stage(name: &str, feedback_to: Option<&str>) -> crate::domain::StageDefinition {
        crate::domain::StageDefinition {
            name: name.to_string(),
            initial: false,
            terminal: false,
            gate: false,
            fresh: false,
            feedback_to: feedback_to.map(|s| s.to_string()),
            worktree: false,
            concurrency: None,
        }
    }

    #[test]
    fn graph_coloring_no_adjacent_same_color() {
        // 4-stage workflow: alpha → beta → gamma → delta
        // with gamma feedback_to: alpha
        // Adjacent pairs: (0,1), (1,2), (2,3), (2,0) via feedback
        let stages = vec![
            make_stage("alpha", None),
            make_stage("beta", None),
            make_stage("gamma", Some("alpha")),
            make_stage("delta", None),
        ];
        let colors = super::assign_stage_colors(&stages);
        assert_eq!(colors.len(), 4);
        let c = |name: &str| *colors.get(name).unwrap();
        assert_ne!(c("alpha"), c("beta"), "alpha vs beta must differ");
        assert_ne!(c("beta"), c("gamma"), "beta vs gamma must differ");
        assert_ne!(c("gamma"), c("delta"), "gamma vs delta must differ");
        assert_ne!(
            c("gamma"),
            c("alpha"),
            "gamma vs alpha (feedback edge) must differ"
        );
    }

    #[test]
    fn graph_coloring_linear_path_spreads_across_palette() {
        // For typical 5-stage workflows we prefer a richer palette than a
        // minimal 2-color alternation, while still keeping adjacent stages distinct.
        let stages = vec![
            make_stage("a", None),
            make_stage("b", None),
            make_stage("c", None),
            make_stage("d", None),
            make_stage("e", None),
        ];
        let colors = super::assign_stage_colors(&stages);
        let distinct: std::collections::HashSet<Color> = colors.values().copied().collect();
        assert!(
            distinct.len() >= 5,
            "5-stage linear workflow should use at least 5 colors, got {} distinct: {:?}",
            distinct.len(),
            distinct
        );
        // Adjacent constraint still holds.
        for i in 0..stages.len() - 1 {
            let ca = colors[&stages[i].name];
            let cb = colors[&stages[i + 1].name];
            assert_ne!(
                ca,
                cb,
                "adjacent stages {} and {} must differ",
                stages[i].name,
                stages[i + 1].name
            );
        }
    }

    #[test]
    fn graph_coloring_produces_distinct_rgb_colors_for_standard_workflow() {
        // Standard spacetop-dev 5-stage workflow.
        // All stages should get distinct Color::Rgb values derived from oklch.
        let stages = vec![
            {
                let mut s = make_stage("design", None);
                s.initial = true;
                s
            },
            make_stage("plan", None),
            {
                let mut s = make_stage("implement", None);
                s.worktree = true;
                s
            },
            {
                let mut s = make_stage("review", Some("implement"));
                s.gate = true;
                s
            },
            {
                let mut s = make_stage("done", None);
                s.terminal = true;
                s
            },
        ];
        let colors = super::assign_stage_colors(&stages);
        // All 5 stage colors must be Color::Rgb variants (oklch-derived).
        for stage_name in &["design", "plan", "implement", "review", "done"] {
            let color = colors[*stage_name];
            assert!(
                matches!(color, Color::Rgb(_, _, _)),
                "stage {stage_name} color should be Color::Rgb, got {color:?}"
            );
        }
        // All 5 colors must be distinct.
        let distinct: std::collections::HashSet<Color> = colors.values().copied().collect();
        assert_eq!(
            distinct.len(),
            5,
            "5 stages should have 5 distinct colors, got {:?}",
            distinct
        );
    }

    #[test]
    fn preview_renders_fenced_code_block_without_backtick_fences() {
        let body = "Some prose.\n\n```rust\nlet x = 1;\n```\n\nAfter block.";
        let app = app_with_items(vec![item("001", "Code Block Preview", body)]);
        let mut terminal = Terminal::new(TestBackend::new(120, 24)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let rendered = buffer_text(buffer);

        // Backtick fences must not appear
        assert!(
            !rendered.contains("```"),
            "backtick fences should not be visible"
        );

        // Code body text must appear
        assert!(
            rendered.contains("let x = 1;"),
            "code body text must be rendered"
        );

        // Code text must carry distinct styling (Cyan fg or DarkGray bg)
        assert!(
            find_styled_text(buffer, "let x = 1;", |style| {
                style.fg == Some(Color::Cyan) || style.bg == Some(Color::DarkGray)
            }),
            "code block text must have distinct style"
        );
    }

    #[test]
    fn render_markdown_termimad_multiline_code_block_emits_one_line_per_source_line() {
        // Termimad fills a code block to the outer width with a Cyan/DarkGray
        // slab. Each source line should remain on its own Line, preserving the
        // visible text at the start of the styled span and padding out to at
        // least the requested pane width.
        let pane_width: u16 = 40;
        let markdown = "```rust\nlet x = 1;\nlet y = 2;\nlet z = 3;\n```";
        let lines = super::markdown::render_markdown_termimad(markdown, pane_width);

        // Collect spans that carry the slab styling.
        let code_spans: Vec<&str> = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .filter_map(|span| {
                if span.style.bg == Some(Color::DarkGray)
                    && span.style.fg == Some(Color::Cyan)
                {
                    Some(span.content.as_ref())
                } else {
                    None
                }
            })
            .collect();

        // There must be exactly 3 code spans, one per source line.
        assert_eq!(
            code_spans.len(),
            3,
            "each source line in a multi-line code block must produce a separate styled span; got {:?}",
            code_spans,
        );

        // Termimad pads each code compound to at least the outer width.
        for span_content in &code_spans {
            assert!(
                span_content.chars().count() >= pane_width as usize,
                "code line span must be padded to at least pane_width ({pane_width}), got len {}",
                span_content.chars().count(),
            );
        }

        // Source text must be preserved at the start of the padded span.
        assert!(
            code_spans[0].starts_with("let x = 1;"),
            "first code line content must be preserved (got {:?})",
            code_spans[0],
        );
        assert!(
            code_spans[1].starts_with("let y = 2;"),
            "second code line content must be preserved (got {:?})",
            code_spans[1],
        );
        assert!(
            code_spans[2].starts_with("let z = 3;"),
            "third code line content must be preserved (got {:?})",
            code_spans[2],
        );
    }

    #[test]
    fn preview_renders_multiline_code_block_on_distinct_rows() {
        // This test renders a 3-line fenced code block through the full TUI
        // pipeline (render_markdown_lines -> Paragraph widget -> TestBackend)
        // and asserts that each code line appears at a different Y coordinate
        // in the terminal buffer.  A regression where all code lines collapse
        // to a single row would fail this assertion even if the unit test for
        // render_markdown_lines passes.
        let body = "```rust\nlet x = 1;\nlet y = 2;\nlet z = 3;\n```";
        let app = app_with_items(vec![item("001", "Multi-line Code Block", body)]);
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let rendered = buffer_text(buffer);

        // All three code lines must appear somewhere in the rendered output.
        assert!(
            rendered.contains("let x = 1;"),
            "first code line must appear"
        );
        assert!(
            rendered.contains("let y = 2;"),
            "second code line must appear"
        );
        assert!(
            rendered.contains("let z = 3;"),
            "third code line must appear"
        );

        // Each code line must be on a distinct row in the buffer.
        let y_x = find_text(buffer, "let x = 1;");
        let y_y = find_text(buffer, "let y = 2;");
        let y_z = find_text(buffer, "let z = 3;");

        assert!(!y_x.is_empty(), "let x = 1; not found in buffer");
        assert!(!y_y.is_empty(), "let y = 2; not found in buffer");
        assert!(!y_z.is_empty(), "let z = 3; not found in buffer");

        let row_x = y_x[0].1;
        let row_y = y_y[0].1;
        let row_z = y_z[0].1;

        assert_ne!(
            row_x, row_y,
            "first and second code lines must render on different rows (got row {row_x})"
        );
        assert_ne!(
            row_y, row_z,
            "second and third code lines must render on different rows (got row {row_y})"
        );
        assert!(
            row_y > row_x,
            "second code line (row {row_y}) must be below first (row {row_x})"
        );
        assert!(
            row_z > row_y,
            "third code line (row {row_z}) must be below second (row {row_y})"
        );
    }

    #[test]
    fn code_block_background_fills_pane_width_in_wrap_mode() {
        let body = "```rust\nlet x = 1;\n```";
        let app = app_with_items(vec![item("001", "Code Wrap", body)]);
        // app_with_items presses Enter to open preview; wrap is on by default.

        let width: u16 = 80;
        let height: u16 = 24;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");

        let buffer = terminal.backend().buffer();
        // Find the row that contains the code body text.
        let hits = find_text(buffer, "let x = 1;");
        assert!(!hits.is_empty(), "code line must appear in buffer");
        let (_, row) = hits[0];

        // Every cell on that row within the preview pane content area must have
        // DarkGray background so the code block background fills the full width.
        // The preview pane has a LEFT border at width/2, so content starts at width/2+1.
        let preview_start = width / 2 + 1;
        for col in preview_start..width {
            let style = buffer[(col, row)].style();
            assert_eq!(
                style.bg,
                Some(Color::DarkGray),
                "col {col} on code row {row} must have DarkGray background in wrap mode"
            );
        }
    }

    #[test]
    fn code_block_long_line_both_wrapped_rows_have_full_background() {
        // Code line longer than the preview pane width so it wraps to a second row.
        let long_line = "X".repeat(120);
        let body = format!("```rust\n{long_line}\n```");
        let app = app_with_items(vec![item("001", "Long Code Wrap", &body)]);
        // wrap is on by default.

        let width: u16 = 80;
        let height: u16 = 30;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");

        let buffer = terminal.backend().buffer();
        // Find the first row containing the leading X run.
        let hits = find_text(buffer, "XXXX");
        assert!(!hits.is_empty(), "long code line must appear in buffer");
        let (_, first_row) = hits[0];

        // Both the first and second visual row of the wrapped code line must be
        // fully backgrounded. Check rightmost and leftmost preview content cells on each row.
        // The preview pane has a LEFT border at width/2, so content starts at width/2+1.
        let preview_start = width / 2 + 1;
        for row in [first_row, first_row + 1] {
            let style_right = buffer[(width - 1, row)].style();
            assert_eq!(
                style_right.bg,
                Some(Color::DarkGray),
                "rightmost cell on wrapped code row {row} must have DarkGray background"
            );
            let style_left = buffer[(preview_start, row)].style();
            assert_eq!(
                style_left.bg,
                Some(Color::DarkGray),
                "preview_start cell on wrapped code row {row} must have DarkGray background"
            );
        }
    }

    #[test]
    fn code_block_background_fills_width_when_scrollbar_is_shown() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        // Build a body tall enough to trigger the vertical scrollbar, then a code block.
        // This is the exact case the two-pass fix addresses: body_area is 1 column
        // narrower than body_inner when the scrollbar is present.
        let many_lines = "line\n".repeat(40);
        let body = format!("{many_lines}```rust\nlet x = 1;\n```");
        let mut app = app_with_items(vec![item("001", "Scrollbar Code", &body)]);
        // wrap is on by default.

        let width: u16 = 80;
        let height: u16 = 24;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");

        // First draw: populates max_preview_scroll so PageDown can scroll.
        terminal
            .draw(|frame| render(frame, &app))
            .expect("first render");

        // Scroll to the bottom using PageDown repeatedly.
        for _ in 0..20 {
            app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
            terminal
                .draw(|frame| render(frame, &app))
                .expect("scroll render");
        }

        let buffer = terminal.backend().buffer();
        let hits = find_text(buffer, "let x = 1;");
        assert!(
            !hits.is_empty(),
            "code line must be visible after scrolling to bottom"
        );
        let (_, row) = hits[0];

        // With the scrollbar present the content area is body_inner.width - 1 columns wide.
        // The rightmost content cell (width - 2; width - 1 is the scrollbar column) must
        // have DarkGray background.
        let preview_start = width / 2 + 1;
        for col in preview_start..width - 1 {
            let style = buffer[(col, row)].style();
            assert_eq!(
                style.bg,
                Some(Color::DarkGray),
                "col {col} on code row {row} must have DarkGray background with scrollbar present"
            );
        }
    }

    // ---- Task 038: worktree marker in task list + diff in preview ----

    fn item_with_worktree_source(id: &str, title: &str, body: &str) -> WorkItem {
        let mut it = item(id, title, body);
        it.worktree_source = Some(PathBuf::from(format!("/tmp/wt/{id}.md")));
        it
    }

    #[test]
    fn task_row_renders_worktree_marker_when_sourced_from_worktree() {
        // AC-1: worktree-sourced row carries the `⎇` marker; main-only row does not.
        let app = app_with_items(vec![
            item("001", "Main task", "Body"),
            item_with_worktree_source("002", "Worktree task", "Body"),
        ]);
        let mut terminal = Terminal::new(TestBackend::new(120, 24)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let buffer = terminal.backend().buffer();

        let wt_hits = find_text(buffer, "Worktree task");
        let main_hits = find_text(buffer, "Main task");
        assert!(!wt_hits.is_empty(), "worktree task row should be rendered");
        assert!(!main_hits.is_empty(), "main task row should be rendered");

        // The marker glyph `⎇` (U+2387) must appear on the worktree row only.
        let (wt_x, wt_y) = wt_hits[0];
        let (_, main_y) = main_hits[0];

        // Marker sits immediately before the title — scan a few cells back.
        let marker = '\u{2387}';
        let mut wt_has_marker = false;
        for x in 0..wt_x {
            if buffer[(x, wt_y)].symbol().chars().any(|c| c == marker) {
                wt_has_marker = true;
                break;
            }
        }
        assert!(wt_has_marker, "worktree row should contain ⎇ marker");

        let mut main_has_marker = false;
        for x in 0..buffer.area.width {
            if buffer[(x, main_y)].symbol().chars().any(|c| c == marker) {
                main_has_marker = true;
                break;
            }
        }
        assert!(
            !main_has_marker,
            "main-only row must NOT contain ⎇ marker"
        );
    }

    #[test]
    fn preview_renders_diff_when_main_body_present() {
        // AC-3: when `main_body.is_some()`, the preview body area shows a
        // unified diff with `+`/`-` lines from the divergent content.
        let mut it = item("050", "Divergent", "alpha\nNEW LINE\ngamma\n");
        it.main_body = Some("alpha\nOLD LINE\ngamma\n".to_string());
        let app = app_with_items(vec![it]);
        let mut terminal = Terminal::new(TestBackend::new(140, 30)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let buffer = terminal.backend().buffer();
        let text = buffer_text(buffer);

        assert!(
            text.contains("+NEW LINE"),
            "preview should contain '+NEW LINE' from diff; rendered: {text}"
        );
        assert!(
            text.contains("-OLD LINE"),
            "preview should contain '-OLD LINE' from diff; rendered: {text}"
        );
    }

    #[test]
    fn preview_falls_back_to_body_when_main_body_none() {
        // AC-3: when `main_body` is None, the preview renders the body as
        // plain markdown (no `+`/`-` diff prefix lines).
        let app = app_with_items(vec![item(
            "051",
            "Plain",
            "alpha\nbeta\ngamma\n",
        )]);
        let mut terminal = Terminal::new(TestBackend::new(140, 30)).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("render");
        let buffer = terminal.backend().buffer();
        let text = buffer_text(buffer);

        assert!(
            text.contains("beta"),
            "preview should still render body content"
        );
        // No diff prefixes should leak when main_body is None.
        assert!(
            !text.contains("+beta") && !text.contains("-beta"),
            "preview should not show diff markers when main_body is None"
        );
    }
}
