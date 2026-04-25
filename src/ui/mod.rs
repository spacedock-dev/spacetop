mod graph;
mod picker;

use crossterm::event::Event;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, AppMode, OverviewSession, OverviewState, ViewScope};
use graph::render_stage_graph_with_breadcrumb;

pub type TerminalEvent = Event;

/// Width cap for the centered dashboard column. On terminals wider than this,
/// the overview content is centered with equal margins on either side.
const MAX_CONTENT_WIDTH: u16 = 120;

pub fn render_placeholder(frame: &mut Frame<'_>) {
    frame.render_widget(
        Paragraph::new("SpaceTop workflow overview is not implemented yet."),
        frame.area(),
    );
}

pub fn render(frame: &mut Frame<'_>, app: &App) {
    match app.mode() {
        AppMode::Picker(state) => {
            let inner = centered_column(frame.area());
            picker::render_in(frame, inner, state);
        }
        AppMode::Overview(session) => {
            let inner = centered_column(frame.area());
            render_overview(frame, inner, session);
        }
        AppMode::PickerOverlay { underlying, picker } => {
            // Draw the underlying overview first, then overlay the picker
            // atop a `Clear` widget over the same centered column.
            let inner = centered_column(frame.area());
            render_overview(frame, inner, underlying);
            frame.render_widget(Clear, inner);
            picker::render_in(frame, inner, picker);
        }
    }

    if app.help_open() {
        render_help_popup(frame, frame.area(), app);
    }
}

/// Cap the dashboard column at [`MAX_CONTENT_WIDTH`] and center it inside the
/// available frame width. Narrower terminals just get the full width back.
pub(crate) fn centered_column(area: Rect) -> Rect {
    if area.width <= MAX_CONTENT_WIDTH {
        return area;
    }
    let extra = area.width - MAX_CONTENT_WIDTH;
    let left = extra / 2;
    Rect {
        x: area.x + left,
        y: area.y,
        width: MAX_CONTENT_WIDTH,
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
    let [graph_area, content_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .areas(area);
    let [list_area, preview_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .areas(content_area);

    let breadcrumb = session.breadcrumb_label();
    render_stage_graph_with_breadcrumb(frame, graph_area, state, breadcrumb.as_deref());
    frame.render_widget(task_list(state), list_area);
    frame.render_widget(preview(state), preview_area);
}

fn render_help_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let is_multi = app.as_session().map(|s| s.is_multi()).unwrap_or(false);
    // Center a 60×N popup (or smaller for tiny terminals). Grow for the
    // multi-workflow extra entries.
    let popup_w = area.width.min(60);
    let popup_h = area.height.min(if is_multi { 19 } else { 16 });
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
        Line::from("  Enter          open workflow (picker)"),
        Line::from("  a              toggle active / archived view"),
        Line::from("  ?              toggle this help popup"),
        Line::from("  Esc / q        quit (or close help)"),
    ];
    if is_multi {
        lines.push(Line::from("  ]              cycle to next workflow"));
        lines.push(Line::from("  [              cycle to previous workflow"));
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

fn task_list(app: &OverviewState) -> Paragraph<'_> {
    let scope = app.view_scope();
    let title = match scope {
        ViewScope::Active => "Tasks",
        ViewScope::Archived => "Archived",
    };
    let items = app.visible_items();
    let lines: Vec<Line<'_>> = if items.is_empty() {
        let empty_text = match (scope, app.archive_error()) {
            (ViewScope::Archived, Some(err)) => format!("archive load failed: {err}"),
            (ViewScope::Archived, None) => "No archived items found.".to_string(),
            (ViewScope::Active, _) => "No work items found.".to_string(),
        };
        vec![Line::from(empty_text)]
    } else {
        items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let selected = index == app.selected_index();
                let marker = if selected { ">" } else { " " };
                let prefix = format!("{marker} {} ", item.id);
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

                let base_style = if selected {
                    Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
                } else if scope == ViewScope::Archived {
                    Style::default().add_modifier(Modifier::DIM)
                } else {
                    Style::default()
                };
                let stage_style = if selected {
                    base_style
                } else {
                    Style::default()
                        .fg(stage_color(&item.status))
                        .add_modifier(Modifier::BOLD)
                };

                Line::from(vec![
                    Span::styled(prefix, base_style),
                    Span::styled(bracket, stage_style),
                    Span::styled(suffix, base_style),
                ])
            })
            .collect()
    };

    Paragraph::new(lines)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: true })
}

fn preview(app: &OverviewState) -> Paragraph<'_> {
    let Some(item) = app.selected_item() else {
        return Paragraph::new("Select a work item to inspect it.")
            .block(Block::default().title("Preview").borders(Borders::ALL));
    };

    let score = item
        .score
        .map(|score| format!("{score:.2}"))
        .unwrap_or_else(|| "n/a".to_string());
    let source = item.source.as_deref().unwrap_or("n/a");
    let body_excerpt = item
        .body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("\n");

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
    if app.view_scope() == ViewScope::Archived {
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
    lines.push(Line::from(body_excerpt));

    Paragraph::new(lines)
        .block(Block::default().title("Preview").borders(Borders::ALL))
        .wrap(Wrap { trim: true })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::{backend::TestBackend, layout::Rect, style::Color, Terminal};

    use super::render;
    use crate::app::App;

    #[test]
    fn renders_real_workflow_summary_task_list_and_preview() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
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

    fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("")
    }

    // --- AC-1: help popup ---

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

    // --- AC-2: centered dashboard column on wide terminals ---

    #[test]
    fn dashboard_is_centered_on_wide_terminals() {
        let area = Rect::new(0, 0, 200, 40);
        let inner = super::centered_column(area);
        assert!(inner.width <= super::MAX_CONTENT_WIDTH);
        let left = inner.x - area.x;
        let right = (area.x + area.width) - (inner.x + inner.width);
        // Left and right margins should be roughly equal (within 1 col).
        assert!(
            (left as i32 - right as i32).abs() <= 1,
            "asymmetric centering: left={left} right={right}"
        );
        assert!(left > 0, "expected non-zero left margin on wide terminal");
    }

    #[test]
    fn dashboard_uses_full_width_on_narrow_terminals() {
        let area = Rect::new(0, 0, 80, 30);
        let inner = super::centered_column(area);
        assert_eq!(inner, area);
    }

    #[test]
    fn wide_terminal_render_leaves_left_margin_blank_in_overview() {
        // Render the full app at a wide size — the leftmost columns should be
        // blank because the dashboard is centered.
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
        let width: u16 = 200;
        let height: u16 = 30;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();

        // Column 0 across all rows must be entirely blank when centered.
        for y in 0..height {
            let cell = &buffer[(0, y)];
            assert_eq!(
                cell.symbol(),
                " ",
                "expected blank left margin at (0,{y}), got {:?}",
                cell.symbol()
            );
        }
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

        // Each known stage should have at least one cell whose fg matches its
        // stage_color. That demonstrates per-stage colorization across the
        // ribbon (AC-3 evidence).
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
        // At least 3 distinct stage colors should be present in the rendered
        // buffer — guaranteed by the ribbon coloring distinct stages.
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
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
        let selected = app.selected_item().expect("selected").clone();
        let expected = super::stage_color(&selected.status);
        let status_value = selected.status.clone();
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let buffer = terminal.backend().buffer();
        // Walk every row by COLUMN (not by byte) so multi-byte symbols like `│`
        // in pane borders don't shift the offsets we use to index cells.
        // Build a Vec of per-column symbols, then sliding-window match the
        // literal "status: " followed by the unstyled status value all in the
        // expected stage fg color. This is robust to: body-excerpt
        // repetitions of "status:" (those won't carry the stage fg), border
        // glyphs preceding the label, and Wrap{trim:true} leading-space
        // changes.
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
                // Match label literally.
                let label_ok = label_chars
                    .iter()
                    .enumerate()
                    .all(|(i, &c)| row_syms[start + i] == c);
                if !label_ok {
                    continue;
                }
                // Match status value literally and require the stage fg color.
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
    fn help_popup_shows_cycle_hints_only_in_multi() {
        use crate::app::{OverviewSession, OverviewState};
        use crate::discovery::DiscoveredWorkflow;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        // Multi-workflow session: build two synthetic single-workflow
        // states wrapped in a session via `from_discovery`. The discovery
        // path values aren't dereferenced for help rendering.
        let snap = {
            use crate::domain::{StageDefinition, WorkflowDefinition, WorkflowSnapshot};
            WorkflowSnapshot {
                definition: WorkflowDefinition {
                    root: PathBuf::from("/x/a"),
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
            }
        };
        let initial = OverviewState::from_snapshot(PathBuf::from("/x/a"), snap);
        let discovery = vec![
            DiscoveredWorkflow {
                root: PathBuf::from("/x/a"),
                title: Some("A".into()),
            },
            DiscoveredWorkflow {
                root: PathBuf::from("/x/b"),
                title: Some("B".into()),
            },
        ];
        let session = OverviewSession::from_discovery(PathBuf::from("/x"), discovery, 0, initial);
        let mut app = App::from_session(session);
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("cycle to next workflow"),
            "multi help should include cycle hint"
        );
        assert!(
            rendered.contains("re-discover"),
            "multi help should mention re-discover"
        );

        // Single-workflow session: the existing `App::load` path produces
        // a pinned single session whose help omits cycle hints.
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let mut app = App::load(root).expect("workflow should load");
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            !rendered.contains("cycle to next workflow"),
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
        // All five canonical stages must produce distinct colors.
        let all = [design, plan, implement, review, done];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b, "stage colors should be distinct");
            }
        }
        // Done is green (sanity-check the convention, supports AC-3 evidence).
        assert_eq!(done, Color::Green);
    }
}
