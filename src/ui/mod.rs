mod graph;
mod picker;

use crossterm::event::Event;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, AppMode, OverviewSession, OverviewState, ViewScope};
use graph::render_stage_graph;

pub type TerminalEvent = Event;

/// Target inner width (in cells) used to horizontally center content blocks
/// inside individual panes — task list rows and the preview block. Picked so
/// the visible column-block stops hugging the left edge on wide terminals
/// without wasting too much horizontal real estate. The dashboard pane
/// itself fills the terminal width; this constant only governs the inner
/// content column.
const PANE_CONTENT_TARGET: u16 = 70;

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

/// Center a child rect of width `target_width` (capped at `outer.width`) inside
/// `outer`, preserving full height. Used for centering the column-block of
/// content inside a pane without changing the pane block itself.
fn center_horizontal(outer: Rect, target_width: u16) -> Rect {
    let w = target_width.min(outer.width);
    let left = (outer.width.saturating_sub(w)) / 2;
    Rect {
        x: outer.x + left,
        y: outer.y,
        width: w,
        height: outer.height,
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
    // Vertical layout: optional tab strip (3 lines incl. borders), graph
    // ribbon (7), main content fills the rest, status footer (1 line).
    let constraints: Vec<Constraint> = if show_tabs {
        vec![
            Constraint::Length(3), // tab bar
            Constraint::Length(7), // graph
            Constraint::Min(0),    // content
            Constraint::Length(1), // footer
        ]
    } else {
        vec![
            Constraint::Length(7),
            Constraint::Min(0),
            Constraint::Length(1),
        ]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let (graph_area, content_area, footer_area) = if show_tabs {
        render_tab_bar(frame, chunks[0], session);
        (chunks[1], chunks[2], chunks[3])
    } else {
        (chunks[0], chunks[1], chunks[2])
    };

    render_stage_graph(frame, graph_area, state);

    let [list_area, preview_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .areas(content_area);

    render_task_list(frame, list_area, state);
    render_preview(frame, preview_area, state);

    render_status_footer(frame, footer_area, session);
}

/// Render the workflow tab bar at the top of the dashboard. One tab per
/// discovered workflow; the active tab is highlighted with the implement-
/// stage color and bold/reversed style, others are dimmed. The strip suffix
/// shows the total count, e.g. `(2/5)`, satisfying the captain's "see how
/// many workflows in this repo" request.
fn render_tab_bar(frame: &mut Frame<'_>, area: Rect, session: &OverviewSession) {
    let active = session.active_index();
    let total = session.len();
    let mut spans: Vec<Span<'_>> = Vec::new();
    for (idx, disc) in session.discovery().iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(" "));
        }
        let label = match &disc.title {
            Some(t) if !t.trim().is_empty() => format!(" {t} "),
            _ => disc
                .root
                .file_name()
                .map(|s| format!(" {} ", s.to_string_lossy()))
                .unwrap_or_else(|| format!(" {} ", disc.root.display())),
        };
        let style = if idx == active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        spans.push(Span::styled(label, style));
    }
    let title = format!("Workflows ({}/{})", active + 1, total);
    let para = Paragraph::new(Line::from(spans))
        .block(Block::default().title(title).borders(Borders::ALL));
    frame.render_widget(para, area);
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
    // Render the Block (borders + title) on the full pane width, then
    // render the row column-block centered horizontally inside the pane.
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = build_task_list_lines(state);
    let inner_centered = center_horizontal(inner, PANE_CONTENT_TARGET);
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner_centered);
}

fn build_task_list_lines(state: &OverviewState) -> Vec<Line<'_>> {
    let scope = state.view_scope();
    let items = state.visible_items();
    if items.is_empty() {
        let empty_text = match (scope, state.archive_error()) {
            (ViewScope::Archived, Some(err)) => format!("archive load failed: {err}"),
            (ViewScope::Archived, None) => "No archived items found.".to_string(),
            (ViewScope::Active, _) => "No work items found.".to_string(),
        };
        return vec![Line::from(empty_text)];
    }
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let selected = index == state.selected_index();
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
}

fn render_preview(frame: &mut Frame<'_>, area: Rect, state: &OverviewState) {
    let block = Block::default().title("Preview").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = build_preview_lines(state);
    let inner_centered = center_horizontal(inner, PANE_CONTENT_TARGET);
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner_centered);
}

fn build_preview_lines(state: &OverviewState) -> Vec<Line<'_>> {
    let Some(item) = state.selected_item() else {
        return vec![Line::from("Select a work item to inspect it.")];
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
    lines.push(Line::from(body_excerpt));
    lines
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::{backend::TestBackend, style::Color, Terminal};

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
            rendered.contains("Workflows (1/3)"),
            "tab strip must show count, got render snippet:\n{rendered}"
        );
        for i in 0..3 {
            assert!(
                rendered.contains(&format!("Workflow{i}")),
                "tab bar missing workflow tab #{i}"
            );
        }
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
