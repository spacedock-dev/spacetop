mod graph;
mod picker;

use crossterm::event::Event;
use pulldown_cmark::{Event as MarkdownEvent, Options, Parser, Tag, TagEnd};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, Tabs, Wrap,
    },
};

use crate::app::{App, AppMode, OverviewSession, OverviewState, ViewScope};
use graph::render_stage_graph;

pub type TerminalEvent = Event;

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
            let inner = picker_centered(frame.area());
            picker::render_in(frame, inner, state);
        }
        AppMode::Overview(session) => {
            render_overview(frame, frame.area(), session);
        }
        AppMode::PickerOverlay { underlying, picker } => {
            // Draw the underlying overview at full width, then overlay a
            // centered picker dialog atop a `Clear` widget.
            render_overview(frame, frame.area(), underlying);
            let inner = picker_centered(frame.area());
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
fn picker_centered(area: Rect) -> Rect {
    const PICKER_WIDTH: u16 = 100;
    if area.width <= PICKER_WIDTH {
        return area;
    }
    let extra = area.width - PICKER_WIDTH;
    let left = extra / 2;
    Rect {
        x: area.x + left,
        y: area.y,
        width: PICKER_WIDTH,
        height: area.height,
    }
}

/// Map a stage name to a stable color. Recognises the conventional Spacedock
/// stage names; falls back to a deterministic palette index for anything else
/// so unknown workflows still get distinct colors per stage.
pub(crate) fn stage_color(stage_name: &str) -> Color {
    match stage_name {
        "design" => Color::Blue,
        "plan" => Color::Cyan,
        "implement" => Color::Yellow,
        "review" | "feedback" => Color::Magenta,
        "done" | "complete" | "completed" | "shipped" => Color::Green,
        "blocked" | "rejected" | "failed" => Color::Red,
        other => {
            // Deterministic fallback — sum bytes mod palette length.
            const PALETTE: &[Color] = &[
                Color::Blue,
                Color::Cyan,
                Color::Yellow,
                Color::Magenta,
                Color::Green,
                Color::LightBlue,
                Color::LightMagenta,
            ];
            let idx = other
                .bytes()
                .fold(0usize, |a, b| a.wrapping_add(b as usize))
                % PALETTE.len();
            PALETTE[idx]
        }
    }
}

fn render_overview(frame: &mut Frame<'_>, area: Rect, session: &OverviewSession) {
    let state = session.active_state();
    let show_tabs = session.is_multi();
    let dashboard_area = if show_tabs {
        render_workflow_tabs_panel(frame, area, session)
    } else {
        area
    };

    // Vertical layout inside the active workflow panel: graph ribbon (7),
    // main content fills the rest, status footer (1 line).
    let constraints = vec![
        Constraint::Length(7),
        Constraint::Min(0),
        Constraint::Length(1),
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(dashboard_area);

    let graph_area = chunks[0];
    let content_area = chunks[1];
    let footer_area = chunks[2];
    render_stage_graph(frame, graph_area, state);

    let [list_area, preview_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .areas(content_area);

    render_task_list(frame, list_area, state);
    render_preview(frame, preview_area, state);

    render_status_footer(frame, footer_area, session);
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

/// One-line status footer at the bottom of the dashboard. Surfaces the
/// headline keys so the help popup is discoverable without tutorialising the
/// user. The exact key list adapts to single vs multi sessions.
fn render_status_footer(frame: &mut Frame<'_>, area: Rect, session: &OverviewSession) {
    let mut hints = vec!["?: help"];
    if session.is_multi() {
        hints.push("\u{2190}/\u{2192}: switch workflow");
        hints.push("P: pick");
    }
    hints.push("a: archive");
    hints.push("PgUp/PgDn: preview");
    hints.push("q: quit");
    let text = hints.join("   ");
    let para = Paragraph::new(Line::from(Span::styled(
        text,
        Style::default().add_modifier(Modifier::DIM),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(para, area);
}

fn render_help_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let is_multi = app.as_session().map(|s| s.is_multi()).unwrap_or(false);
    let popup_w = area.width.min(64);
    let popup_h = area.height.min(if is_multi { 20 } else { 16 });
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
        Line::from("  PageUp         scroll preview up"),
        Line::from("  PageDown       scroll preview down"),
        Line::from("  Enter          open workflow (picker)"),
        Line::from("  a              toggle active / archived view"),
        Line::from("  ?              toggle this help popup"),
        Line::from("  Esc / q        quit (or close help)"),
    ];
    if is_multi {
        lines.push(Line::from("  \u{2192} / Right     switch to next workflow"));
        lines.push(Line::from(
            "  \u{2190} / Left      switch to previous workflow",
        ));
        lines.push(Line::from("  P              re-discover & pick workflow"));
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
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let items = build_task_list_items(state);
    let mut list_state = ListState::default().with_selected(if items.is_empty() {
        None
    } else {
        Some(state.selected_index())
    });
    let list = List::new(items)
        .highlight_symbol("> ")
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD));
    frame.render_stateful_widget(list, inner, &mut list_state);
}

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
    items
        .iter()
        .enumerate()
        .map(|(_index, item)| {
            let prefix = format!("{} ", item.id);
            let bracket = format!("[{}]", item.status);
            let suffix = match scope {
                ViewScope::Archived => {
                    let glyph = match item.verdict.as_deref() {
                        Some("PASSED") => "[\u{2713}]",
                        Some(_) => "[\u{2717}]",
                        None => "[?]",
                    };
                    format!(" {} {glyph}", item.title)
                }
                ViewScope::Active => format!(" {}", item.title),
            };

            let prefix_style = if scope == ViewScope::Archived {
                Style::default().add_modifier(Modifier::DIM)
            } else {
                Style::default()
            };
            let title_style = if scope == ViewScope::Archived {
                Style::default().add_modifier(Modifier::DIM)
            } else {
                Style::default()
            };
            let stage_style = Style::default()
                .fg(stage_color(&item.status))
                .add_modifier(Modifier::BOLD);

            ListItem::new(Line::from(vec![
                Span::styled(prefix, prefix_style),
                Span::styled(bracket, stage_style),
                Span::styled(suffix, title_style),
            ]))
        })
        .collect()
}

fn render_preview(frame: &mut Frame<'_>, area: Rect, state: &OverviewState) {
    let block = Block::default().title("Preview").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(item) = state.selected_item() else {
        let paragraph = Paragraph::new("Select a work item to inspect it.");
        frame.render_widget(paragraph, inner);
        return;
    };

    let header_lines = build_preview_header_lines(item, state);
    let header_height = (header_lines.len() as u16).min(inner.height);
    let header_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: header_height,
    };
    frame.render_widget(
        Paragraph::new(header_lines).wrap(Wrap { trim: true }),
        header_area,
    );

    if header_height >= inner.height {
        return;
    }

    let body_inner = Rect {
        x: inner.x,
        y: inner.y + header_height,
        width: inner.width,
        height: inner.height - header_height,
    };
    let blocks = render_markdown_blocks(&item.body, usize::MAX);
    let content_height = blocks.iter().map(MarkdownBlock::height).sum::<u16>();
    let show_scrollbar = content_height > body_inner.height && body_inner.width > 1;
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

    let max_scroll = usize::from(content_height.saturating_sub(body_area.height));
    let scroll_position = state.preview_scroll().min(max_scroll);
    let mut skip_rows = scroll_position as u16;
    let mut cursor_y = body_area.y;
    let mut remaining = body_area.height;
    for block in blocks {
        if remaining == 0 {
            break;
        }
        let block_height = block.height();
        if skip_rows >= block_height {
            skip_rows -= block_height;
            continue;
        }
        let visible_height = (block_height - skip_rows).min(remaining);
        let block_area = Rect {
            x: body_area.x,
            y: cursor_y,
            width: body_area.width,
            height: visible_height,
        };
        match block {
            MarkdownBlock::Paragraph(lines) => {
                frame.render_widget(
                    Paragraph::new(lines)
                        .wrap(Wrap { trim: true })
                        .scroll((skip_rows, 0)),
                    block_area,
                );
            }
            MarkdownBlock::Table(table) => {
                frame.render_widget(table_widget(table.skip_rows(skip_rows)), block_area);
            }
        }
        cursor_y += visible_height;
        remaining -= visible_height;
        skip_rows = 0;
    }

    if show_scrollbar {
        let mut scrollbar_state = ScrollbarState::new(content_height as usize)
            .viewport_content_length(body_inner.height as usize)
            .position(scroll_position);
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
) -> Vec<Line<'a>> {
    let score = item
        .score
        .map(|score| format!("{score:.2}"))
        .unwrap_or_else(|| "n/a".to_string());
    let source = item.source.as_deref().unwrap_or("n/a");
    let mut lines: Vec<Line<'_>> = Vec::new();
    lines.push(Line::from(Span::styled(
        item.title.as_str(),
        Style::default().add_modifier(Modifier::BOLD),
    )));
    let status_color = stage_color(&item.status);
    lines.push(Line::from(vec![
        Span::raw("status: "),
        Span::styled(
            item.status.clone(),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(format!("score: {score}")));
    lines.push(Line::from(format!("source: {source}")));
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
        lines.push(Line::from(vec![
            Span::raw("verdict: "),
            Span::styled(verdict.to_string(), verdict_style),
        ]));
        lines.push(Line::from(format!("completed: {completed}")));
    }
    lines.push(Line::from(format!("path: {}", item.path.display())));
    lines.push(Line::from(""));
    lines
}

#[derive(Debug, Clone)]
enum MarkdownBlock {
    Paragraph(Vec<Line<'static>>),
    Table(TableRender),
}

impl MarkdownBlock {
    fn height(&self) -> u16 {
        match self {
            MarkdownBlock::Paragraph(lines) => lines.len() as u16,
            MarkdownBlock::Table(table) => table.height(),
        }
    }
}

fn render_markdown_blocks(markdown: &str, max_lines: usize) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut text_lines = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut strong = false;
    let mut heading_depth: Option<u32> = None;
    let mut in_item = false;
    let mut table: Option<TableRender> = None;
    let mut table_row: Vec<String> = Vec::new();
    let mut table_cell = String::new();
    let mut in_table_cell = false;

    let parser = Parser::new_ext(markdown, Options::ENABLE_TABLES);
    for event in parser {
        match event {
            MarkdownEvent::Start(Tag::Table(_)) => {
                flush_line(&mut text_lines, &mut spans, max_lines);
                flush_text_block(&mut blocks, &mut text_lines);
                table = Some(TableRender::default());
            }
            MarkdownEvent::End(TagEnd::Table) => {
                if let Some(table) = table.take() {
                    blocks.push(MarkdownBlock::Table(table));
                }
            }
            MarkdownEvent::Start(Tag::TableHead) | MarkdownEvent::Start(Tag::TableRow) => {
                table_row.clear();
            }
            MarkdownEvent::End(TagEnd::TableHead) | MarkdownEvent::End(TagEnd::TableRow) => {
                if let Some(table) = &mut table {
                    table.push_row(std::mem::take(&mut table_row));
                }
            }
            MarkdownEvent::Start(Tag::TableCell) => {
                table_cell.clear();
                in_table_cell = true;
            }
            MarkdownEvent::End(TagEnd::TableCell) => {
                table_row.push(table_cell.trim().to_string());
                table_cell.clear();
                in_table_cell = false;
            }
            MarkdownEvent::Start(Tag::Heading { level, .. }) => {
                flush_line(&mut text_lines, &mut spans, max_lines);
                heading_depth = Some(level as u32);
            }
            MarkdownEvent::End(TagEnd::Heading(_)) => {
                flush_line(&mut text_lines, &mut spans, max_lines);
                flush_text_block(&mut blocks, &mut text_lines);
                heading_depth = None;
            }
            MarkdownEvent::Start(Tag::Paragraph) => {
                if !spans.is_empty() {
                    flush_line(&mut text_lines, &mut spans, max_lines);
                }
            }
            MarkdownEvent::End(TagEnd::Paragraph) => {
                flush_line(&mut text_lines, &mut spans, max_lines);
                flush_text_block(&mut blocks, &mut text_lines);
            }
            MarkdownEvent::Start(Tag::Item) => {
                flush_line(&mut text_lines, &mut spans, max_lines);
                in_item = true;
                spans.push(Span::raw("\u{2022} "));
            }
            MarkdownEvent::End(TagEnd::Item) => {
                flush_line(&mut text_lines, &mut spans, max_lines);
                flush_text_block(&mut blocks, &mut text_lines);
                in_item = false;
            }
            MarkdownEvent::Start(Tag::Strong) => strong = true,
            MarkdownEvent::End(TagEnd::Strong) => strong = false,
            MarkdownEvent::Text(text) => {
                if in_table_cell {
                    table_cell.push_str(&text);
                    continue;
                }
                let mut style = Style::default();
                if strong || heading_depth.is_some() {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if heading_depth.is_some() {
                    style = style.fg(Color::White);
                }
                spans.push(Span::styled(text.to_string(), style));
            }
            MarkdownEvent::Code(text) => {
                if in_table_cell {
                    table_cell.push_str(&text);
                    continue;
                }
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(Color::Yellow),
                ));
            }
            MarkdownEvent::SoftBreak | MarkdownEvent::HardBreak => {
                if in_table_cell {
                    table_cell.push(' ');
                    continue;
                }
                if in_item {
                    spans.push(Span::raw(" "));
                } else {
                    flush_line(&mut text_lines, &mut spans, max_lines);
                }
            }
            MarkdownEvent::Rule => {
                flush_line(&mut text_lines, &mut spans, max_lines);
                if text_lines.len() < max_lines {
                    text_lines.push(Line::from(Span::styled(
                        "\u{2500}".repeat(12),
                        Style::default().add_modifier(Modifier::DIM),
                    )));
                }
                flush_text_block(&mut blocks, &mut text_lines);
            }
            _ => {}
        }

        let used_lines = blocks
            .iter()
            .map(|block| block.height() as usize)
            .sum::<usize>()
            + text_lines.len();
        if used_lines >= max_lines {
            break;
        }
    }

    flush_line(&mut text_lines, &mut spans, max_lines);
    flush_text_block(&mut blocks, &mut text_lines);
    if blocks.is_empty() {
        blocks.push(MarkdownBlock::Paragraph(vec![Line::from("")]));
    }
    add_markdown_block_spacing(blocks)
}

#[derive(Debug, Clone, Default)]
struct TableRender {
    rows: Vec<Vec<String>>,
    widths: Vec<usize>,
}

impl TableRender {
    fn push_row(&mut self, row: Vec<String>) {
        if self.widths.len() < row.len() {
            self.widths.resize(row.len(), 0);
        }
        for (index, cell) in row.iter().enumerate() {
            self.widths[index] = self.widths[index].max(cell.chars().count());
        }
        self.rows.push(row);
    }

    fn height(&self) -> u16 {
        let separator = u16::from(self.rows.len() > 1);
        self.rows.len() as u16 + separator
    }

    fn skip_rows(self, skip: u16) -> Self {
        if skip == 0 {
            return self;
        }

        let mut visual_rows: Vec<Vec<String>> = Vec::new();
        for (index, row) in self.rows.into_iter().enumerate() {
            visual_rows.push(row);
            if index == 0 && self.widths.len() > 0 {
                visual_rows.push(
                    self.widths
                        .iter()
                        .map(|width| "\u{2500}".repeat(*width))
                        .collect(),
                );
            }
        }

        let rows = visual_rows
            .into_iter()
            .skip(skip as usize)
            .collect::<Vec<_>>();
        let mut table = TableRender::default();
        for row in rows {
            table.push_row(row);
        }
        table
    }
}

fn table_widget(table: TableRender) -> Table<'static> {
    let has_body = table.rows.len() > 1;
    let column_widths = table.widths.clone();
    let widths = table
        .widths
        .iter()
        .map(|width| Constraint::Length((*width as u16).max(1)))
        .collect::<Vec<_>>();
    let mut rows = table.rows.into_iter();
    let header = rows.next().map(|row| {
        Row::new(
            row.into_iter()
                .map(|cell| Cell::from(cell).style(Style::default().add_modifier(Modifier::BOLD)))
                .collect::<Vec<_>>(),
        )
    });
    let body_rows = rows.map(|row| {
        Row::new(
            row.into_iter()
                .map(Cell::from)
                .collect::<Vec<Cell<'static>>>(),
        )
    });
    let separator = if header.is_some() && has_body {
        Some(Row::new(
            column_widths
                .iter()
                .map(|width| {
                    Cell::from("\u{2500}".repeat(*width)).style(
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::DIM),
                    )
                })
                .collect::<Vec<_>>(),
        ))
    } else {
        None
    };
    let table_rows = separator.into_iter().chain(body_rows);
    let table = Table::new(table_rows, widths).column_spacing(2);
    if let Some(header) = header {
        table.header(header)
    } else {
        table
    }
}

fn flush_text_block(blocks: &mut Vec<MarkdownBlock>, lines: &mut Vec<Line<'static>>) {
    if !lines.is_empty() {
        blocks.push(MarkdownBlock::Paragraph(std::mem::take(lines)));
    }
}

fn add_markdown_block_spacing(blocks: Vec<MarkdownBlock>) -> Vec<MarkdownBlock> {
    let mut spaced = Vec::with_capacity(blocks.len().saturating_mul(2));
    for (index, block) in blocks.into_iter().enumerate() {
        if index > 0 {
            spaced.push(MarkdownBlock::Paragraph(vec![Line::from("")]));
        }
        spaced.push(block);
    }
    spaced
}

fn flush_line(lines: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>, max_lines: usize) {
    if spans.is_empty() || lines.len() >= max_lines {
        spans.clear();
        return;
    }
    lines.push(Line::from(std::mem::take(spans)));
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::{
        backend::TestBackend,
        style::{Color, Modifier},
        Terminal,
    };

    use super::render;
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
        assert!(rendered.contains(&format!("status: {}", selected.status)));
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
            },
            items,
        };
        App::from_snapshot(root, snapshot)
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
        }
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
        let app = app_with_items(vec![item(
            "001",
            "Markdown Table Preview",
            "Ablation siblings\n| Arm | Entity | README |\n| --- | ---: | --- |\n| 1 | 17 | direct.md |\n| 2 | 18 | method.md |",
        )]);
        let mut terminal = Terminal::new(TestBackend::new(140, 24)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("Ablation siblings"));
        assert!(rendered.contains("Arm  Entity  README"));
        assert!(rendered.contains("1    17      direct.md"));
        assert!(rendered.contains("2    18      method.md"));
        assert!(
            find_styled_text(terminal.backend().buffer(), "Arm", |style| {
                style.add_modifier.contains(ratatui::style::Modifier::BOLD)
            }),
            "table header should be rendered through a highlighted table header row"
        );
        assert!(
            rendered.contains("\u{2500}\u{2500}\u{2500}"),
            "table should show a separator row between header and body"
        );
        assert!(
            !rendered.contains("---") && !rendered.contains("| Arm |"),
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
            find_text_starting_after(buffer, "PREVIEWFULLWIDTH", 150),
            "preview content should use the full preview pane instead of a centered narrow column"
        );
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
        let right_edge = width - 2;
        let has_scrollbar = (1..height - 1).any(|y| {
            let symbol = buffer[(right_edge, y)].symbol();
            symbol == "\u{2588}" || symbol == "\u{2502}"
        });
        assert!(has_scrollbar, "overflowing preview should draw a scrollbar");
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

        assert!(
            find_text_starting_after(buffer, "FULLWIDTHMARKER", 74),
            "task row content should use the whole list pane rather than a centered narrow column"
        );
        let rendered = buffer_text(buffer);
        assert!(
            rendered.contains("> 001"),
            "selected row should use ratatui-style highlight symbol"
        );
        assert!(
            find_styled_text(buffer, stable_title, |style| {
                style
                    .add_modifier
                    .contains(ratatui::style::Modifier::REVERSED)
            }),
            "selected row title should be highlighted by ratatui List selection"
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
        assert!(rendered.contains("Esc / q"), "missing Esc/q binding");
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
        // The Overview block (graph ribbon) must touch column 0 and the
        // last column on a wide terminal — i.e. no left/right margin
        // gutter. This codifies the override of task 009's centered-
        // column rule.
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
        let width: u16 = 200;
        let height: u16 = 30;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();
        // The graph block has a top border drawn at row 0; that border
        // should reach both the left and right edges of the terminal.
        let top_left = buffer[(0, 0)].symbol();
        let top_right = buffer[(width - 1, 0)].symbol();
        assert_ne!(
            top_left, " ",
            "expected non-blank left edge of dashboard at (0,0), got blank"
        );
        assert_ne!(
            top_right,
            " ",
            "expected non-blank right edge of dashboard at ({},0), got blank",
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
        let app = App::from_session(session);
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
        let stage_colors: std::collections::HashSet<Color> = app
            .snapshot()
            .definition
            .stages
            .iter()
            .map(|s| super::stage_color(&s.name))
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
        let label_chars: [&str; 8] = ["s", "t", "a", "t", "u", "s", ":", " "];
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
            rendered.contains('\u{2192}') || rendered.contains("Right"),
            "help popup must list right-arrow binding in multi"
        );
        assert!(
            rendered.contains('\u{2190}') || rendered.contains("Left"),
            "help popup must list left-arrow binding in multi"
        );
        assert!(
            rendered.contains("re-discover"),
            "multi help should mention re-discover"
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
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b, "stage colors should be distinct");
            }
        }
        assert_eq!(done, Color::Green);
    }
}
