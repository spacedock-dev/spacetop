//! Full-pane Workflow Definition view.
//!
//! Renders the active workflow's `WorkflowDefinition` — a Stages table
//! summarising every `StageDefinition` field plus per-stage README
//! prose blocks lifted via `parse_stage_prose` and rendered through
//! the same termimad pipeline the preview pane uses. The view is
//! driven by a scroll offset; no I/O occurs on the render path.
//!
//! Entry point: [`render_in`]. Tests live in this module's `tests`
//! submodule and use `TestBackend` with synthetic
//! `WorkflowDefinition` fixtures.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Frame, Line, Modifier, Span, Style},
    widgets::{Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, Wrap},
};

use spacetop_core::domain::{StageDefinition, WorkflowDefinition};

use super::markdown::render_markdown_termimad;

/// Dim em-dash used when an optional stage field is absent.
const EM_DASH: &str = "\u{2014}";

/// Render the full-pane Workflow Definition view into `area`. The
/// caller (the top-level `ui::render` match arm) hands us the whole
/// frame area; we own header rows, the stages table, and the
/// per-stage prose blocks.
pub fn render_in(
    frame: &mut Frame<'_>,
    area: Rect,
    definition: &WorkflowDefinition,
    scroll: usize,
) {
    // Vertical layout:
    //   Row 0          - header
    //   Row 1          - scope sub-line
    //   Stages table   - 2 + stages.len() rows (header + body + 1 blank below)
    //   Rest           - per-stage prose, scrollable
    let stages_table_height = (definition.stages.len() as u16).saturating_add(2);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(stages_table_height),
            Constraint::Min(0),
        ])
        .split(area);

    render_header(frame, chunks[0], definition);
    render_scope_subline(frame, chunks[1], definition);
    render_stages_table(frame, chunks[2], definition);
    render_prose_body(frame, chunks[3], definition, scroll);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, definition: &WorkflowDefinition) {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let basename = definition
        .root
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| definition.root.display().to_string());
    let plural = definition
        .entity_label_plural
        .clone()
        .unwrap_or_else(|| "entities".to_string());

    let path_full = definition.root.display().to_string();
    let prefix = format!("Workflow Definition  \u{00B7}  {basename}  \u{00B7}  {plural} ");
    let prefix_len = prefix.chars().count();
    let available = (area.width as usize).saturating_sub(prefix_len);
    let path_str = left_fit(&path_full, available);
    let used = prefix_len + path_str.chars().count();
    let trailing_spaces = (area.width as usize).saturating_sub(used);

    let line = Line::from(vec![
        Span::styled("Workflow Definition  \u{00B7}  ", dim),
        Span::styled(basename, Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(format!("  \u{00B7}  {plural} "), dim),
        Span::styled(path_str, dim),
        Span::styled(" ".repeat(trailing_spaces), dim),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_scope_subline(frame: &mut Frame<'_>, area: Rect, definition: &WorkflowDefinition) {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let id_style = definition.id_style.as_deref().unwrap_or(EM_DASH);
    let entity_type = definition.entity_type.as_deref().unwrap_or(EM_DASH);
    let entity_label = definition.entity_label.as_deref().unwrap_or(EM_DASH);
    let line = Line::from(vec![
        Span::styled("id-style: ", dim),
        Span::raw(id_style.to_string()),
        Span::styled("  \u{00B7}  entity-type: ", dim),
        Span::raw(entity_type.to_string()),
        Span::styled("  \u{00B7}  entity-label: ", dim),
        Span::raw(entity_label.to_string()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_stages_table(frame: &mut Frame<'_>, area: Rect, definition: &WorkflowDefinition) {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let header = Row::new(vec![
        Span::styled("Stage", dim),
        Span::styled("Flags", dim),
        Span::styled("Feedback", dim),
        Span::styled("Concurrency", dim),
    ]);

    let rows: Vec<Row<'_>> = definition
        .stages
        .iter()
        .map(|stage| stage_row(stage, definition))
        .collect();

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(40),
        Constraint::Percentage(25),
        Constraint::Percentage(15),
    ];
    let table = Table::new(rows, widths).header(header);
    frame.render_widget(table, area);
}

fn stage_row<'a>(stage: &'a StageDefinition, definition: &'a WorkflowDefinition) -> Row<'a> {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let stage_color = crate::ui::color::to_color(definition.stage_color_for(&stage.name));
    let stage_cell = Line::from(Span::styled(
        stage.name.clone(),
        Style::default()
            .fg(stage_color)
            .add_modifier(Modifier::BOLD),
    ));

    let flags = build_flags_spans(stage);
    let flags_cell = if flags.is_empty() {
        Line::from(Span::styled(EM_DASH, dim))
    } else {
        Line::from(flags)
    };

    let feedback_cell = match stage.feedback_to.as_deref() {
        Some(target) => {
            let target_color = crate::ui::color::to_color(definition.stage_color_for(target));
            Line::from(vec![
                Span::styled("\u{2192} ", dim),
                Span::styled(target.to_string(), Style::default().fg(target_color)),
            ])
        }
        None => Line::from(Span::styled(EM_DASH, dim)),
    };

    let conc_cell = match stage.concurrency {
        Some(n) => Line::from(Span::raw(n.to_string())),
        None => Line::from(Span::styled(EM_DASH, dim)),
    };

    Row::new(vec![stage_cell, flags_cell, feedback_cell, conc_cell])
}

fn build_flags_spans(stage: &StageDefinition) -> Vec<Span<'static>> {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let mut chips: Vec<&'static str> = Vec::new();
    if stage.initial {
        chips.push("initial");
    }
    if stage.terminal {
        chips.push("terminal");
    }
    if stage.gate {
        chips.push("gate");
    }
    if stage.fresh {
        chips.push("fresh");
    }
    if stage.worktree {
        chips.push("worktree");
    }
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(chips.len() * 4);
    for (i, chip) in chips.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled("[", dim));
        spans.push(Span::raw((*chip).to_string()));
        spans.push(Span::styled("]", dim));
    }
    spans
}

fn render_prose_body(
    frame: &mut Frame<'_>,
    area: Rect,
    definition: &WorkflowDefinition,
    scroll: usize,
) {
    if area.height == 0 {
        return;
    }
    let dim = Style::default().add_modifier(Modifier::DIM);

    let mut lines: Vec<Line<'static>> = Vec::new();
    for stage in &definition.stages {
        let stage_color = crate::ui::color::to_color(definition.stage_color_for(&stage.name));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("### {}", stage.name),
            Style::default()
                .fg(stage_color)
                .add_modifier(Modifier::BOLD),
        )));
        match definition.stage_prose.get(&stage.name) {
            Some(body) if !body.trim().is_empty() => {
                let rendered = render_markdown_termimad(body, area.width);
                lines.extend(rendered);
            }
            _ => {
                lines.push(Line::from(Span::styled(
                    "(no description in README)".to_string(),
                    dim,
                )));
            }
        }
    }

    let content_height = lines.len() as u16;
    let show_scrollbar = content_height > area.height && area.width > 1;
    let body_area = if show_scrollbar {
        Rect {
            x: area.x,
            y: area.y,
            width: area.width - 1,
            height: area.height,
        }
    } else {
        area
    };

    let max_scroll = usize::from(content_height.saturating_sub(body_area.height));
    let clamped = scroll.min(max_scroll);
    let paragraph = Paragraph::new(lines)
        .scroll((clamped as u16, 0))
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);
    frame.render_widget(paragraph, body_area);

    if show_scrollbar {
        let mut sb_state = ScrollbarState::new(max_scroll + 1).position(clamped);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("\u{2502}"))
                .thumb_symbol("\u{2588}"),
            area,
            &mut sb_state,
        );
    }
}

/// Truncate `value` from the LEFT to fit in `available` cells. Prefixes
/// with `\u{2026}` (ellipsis) when truncation is needed; returns the
/// value unchanged if it fits, and a single ellipsis when even one cell
/// is too narrow to render anything else.
fn left_fit(value: &str, available: usize) -> String {
    let count = value.chars().count();
    if count <= available {
        return value.to_string();
    }
    if available <= 1 {
        return "\u{2026}".to_string();
    }
    let skip = count - (available - 1);
    let tail: String = value.chars().skip(skip).collect();
    format!("\u{2026}{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, AppMode, OverviewSession, OverviewState};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};
    use spacetop_core::discovery::DiscoveredWorkflow;
    use spacetop_core::domain::{StageDefinition, WorkflowDefinition};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    fn five_stage_fixture(root: PathBuf, prose: HashMap<String, String>) -> WorkflowDefinition {
        let stages = vec![
            StageDefinition {
                name: "design".to_string(),
                initial: true,
                terminal: false,
                gate: false,
                fresh: false,
                feedback_to: None,
                worktree: false,
                concurrency: Some(2),
            },
            StageDefinition {
                name: "plan".to_string(),
                initial: false,
                terminal: false,
                gate: false,
                fresh: false,
                feedback_to: None,
                worktree: false,
                concurrency: Some(2),
            },
            StageDefinition {
                name: "implement".to_string(),
                initial: false,
                terminal: false,
                gate: false,
                fresh: false,
                feedback_to: None,
                worktree: true,
                concurrency: Some(2),
            },
            StageDefinition {
                name: "review".to_string(),
                initial: false,
                terminal: false,
                gate: true,
                fresh: true,
                feedback_to: Some("implement".to_string()),
                worktree: false,
                concurrency: Some(1),
            },
            StageDefinition {
                name: "done".to_string(),
                initial: false,
                terminal: true,
                gate: false,
                fresh: false,
                feedback_to: None,
                worktree: false,
                concurrency: None,
            },
        ];
        let stage_colors = spacetop_core::domain::assign_stage_colors(&stages);
        WorkflowDefinition {
            root,
            stages,
            id_style: Some("sequential".to_string()),
            entity_type: Some("development_task".to_string()),
            entity_label: Some("task".to_string()),
            entity_label_plural: Some("tasks".to_string()),
            stage_colors,
            stage_prose: prose,
            transitions: Vec::new(),
        }
    }

    /// AC-2 + AC-2-header: every flag chip, every feedback arrow / em-dash,
    /// every concurrency value / em-dash, and the workflow-scope fields
    /// must appear in the rendered buffer for a 5-stage fixture
    /// exercising every flag combination.
    #[test]
    fn stages_table_renders_every_stage_field() {
        let definition = five_stage_fixture(PathBuf::from("/workflow-fixture"), HashMap::new());

        let mut terminal = Terminal::new(TestBackend::new(140, 50)).expect("terminal");
        terminal
            .draw(|frame| render_in(frame, frame.area(), &definition, 0))
            .expect("render");
        let rendered = buffer_text(terminal.backend().buffer());

        // Every stage name appears.
        for stage in &definition.stages {
            assert!(
                rendered.contains(stage.name.as_str()),
                "stage {} should appear in buffer; rendered=\n{rendered}",
                stage.name
            );
        }
        // Flag chips appear when set.
        for chip in ["[initial]", "[terminal]", "[gate]", "[fresh]", "[worktree]"] {
            assert!(
                rendered.contains(chip),
                "flag chip {chip} should appear; rendered=\n{rendered}"
            );
        }
        // Feedback arrow + target appear for review→implement.
        assert!(
            rendered.contains("\u{2192} implement"),
            "feedback arrow + target should appear; rendered=\n{rendered}"
        );
        // Em-dash appears for stages without feedback / without concurrency.
        assert!(
            rendered.contains(EM_DASH),
            "em-dash placeholder should appear for empty optional fields; rendered=\n{rendered}"
        );
        // Concurrency numeric values appear.
        for n in ["2", "1"] {
            assert!(
                rendered.contains(n),
                "concurrency value {n} should appear; rendered=\n{rendered}"
            );
        }
    }

    /// AC-2: header carries workflow-scope fields (`id_style`,
    /// `entity_label_plural`).
    #[test]
    fn header_carries_scope_fields() {
        let definition = five_stage_fixture(PathBuf::from("/workflow-fixture"), HashMap::new());
        let mut terminal = Terminal::new(TestBackend::new(140, 50)).expect("terminal");
        terminal
            .draw(|frame| render_in(frame, frame.area(), &definition, 0))
            .expect("render");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("id-style: sequential"));
        assert!(rendered.contains("tasks")); // entity_label_plural
        assert!(rendered.contains("entity-type: development_task"));
    }

    /// AC-3: the rendered view exposes the per-stage prose text from
    /// the `plan` stage when it is present in the `stage_prose` map.
    #[test]
    fn stage_prose_block_appears_in_view() {
        let mut prose = HashMap::new();
        prose.insert(
            "plan".to_string(),
            "Approved design notes plus more context.".to_string(),
        );
        let definition = five_stage_fixture(PathBuf::from("/workflow-fixture"), prose);
        let mut terminal = Terminal::new(TestBackend::new(140, 60)).expect("terminal");
        terminal
            .draw(|frame| render_in(frame, frame.area(), &definition, 0))
            .expect("render");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("Approved design notes"),
            "expected plan prose substring in buffer; rendered=\n{rendered}"
        );
    }

    /// AC-3: a stage with no prose entry renders the dim
    /// "(no description in README)" placeholder.
    #[test]
    fn missing_stage_prose_renders_placeholder() {
        let definition = five_stage_fixture(PathBuf::from("/workflow-fixture"), HashMap::new());
        let mut terminal = Terminal::new(TestBackend::new(140, 60)).expect("terminal");
        terminal
            .draw(|frame| render_in(frame, frame.area(), &definition, 0))
            .expect("render");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("(no description in README)"),
            "expected placeholder; rendered=\n{rendered}"
        );
    }

    /// AC-3 + real fixture: load the real `docs/spacetop-dev/README.md`
    /// through `App::load`, switch to the Definition mode, render, and
    /// assert every frontmatter-declared stage name appears in the
    /// rendered buffer.
    #[test]
    fn definition_renders_against_real_readme() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
        let mut app = App::load(root).expect("load");
        // Press D to enter Definition mode.
        app.handle_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::NONE));
        assert!(app.is_definition(), "D should switch to Definition mode");

        let mut terminal = Terminal::new(TestBackend::new(160, 80)).expect("terminal");
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .expect("render");
        let rendered = buffer_text(terminal.backend().buffer());

        let definition = app.snapshot().definition.clone();
        for stage in &definition.stages {
            assert!(
                rendered.contains(stage.name.as_str()),
                "stage {} should appear in rendered Definition view; rendered=\n{rendered}",
                stage.name
            );
        }
        // The real README's `plan` stage Inputs bullet starts with
        // "Approved shape notes" — assert that propagates through.
        assert!(
            rendered.contains("Approved shape notes"),
            "plan prose substring 'Approved shape notes' must appear; rendered=\n{rendered}"
        );
    }

    /// AC-5: header carries the basename of the active tab when in a
    /// multi-workflow session.
    #[test]
    fn definition_header_carries_active_tab_basename() {
        // Three temp workflows; cycle to middle; press D; assert header
        // shows the middle workflow's basename.
        let holder = TempDir::new().expect("tempdir");
        let mut discovery = Vec::new();
        let mut roots = Vec::new();
        for i in 0..3 {
            let root = holder.path().join(format!("w{i}"));
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(
                root.join("README.md"),
                "---\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: done\n      terminal: true\n---\n",
            )
            .unwrap();
            std::fs::write(
                root.join("task-001.md"),
                format!("---\nid: 001\ntitle: T{i}\nstatus: plan\n---\n\nbody\n"),
            )
            .unwrap();
            roots.push(root.clone());
            discovery.push(DiscoveredWorkflow {
                root,
                title: Some(format!("W{i}")),
            });
        }
        let initial = OverviewState::load(roots[0].clone()).expect("load w0");
        let session =
            OverviewSession::from_discovery(holder.path().to_path_buf(), discovery, 0, initial);
        let mut app = App::from_session(session);
        // Cycle to middle tab.
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        // The cycle emits a pending switch with needs_first_load=true; the
        // event loop normally materialises. Do that explicitly so the new
        // active state is real.
        let _ = app.take_pending_switch();
        app.materialize_active();
        assert_eq!(app.as_session().unwrap().active_index(), 1);

        // D opens the definition.
        app.handle_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::NONE));
        assert!(matches!(app.mode(), AppMode::Definition { .. }));

        let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .expect("render");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("w1"),
            "header should contain middle workflow basename 'w1'; rendered=\n{rendered}"
        );
    }

    /// AC-4: the D-key definition view renders prose for stages whose
    /// `### {stage}` heading uses the qualifier-suffixed form
    /// (`### \`scoping\` (lead only, worktree)`). Mirrors the
    /// `definition_renders_against_real_readme` shape but with a
    /// synthetic README so the failure mode is unambiguously about
    /// qualifier-suffix handling.
    #[test]
    fn definition_renders_qualifier_suffixed_stages() {
        let holder = TempDir::new().expect("tempdir");
        let root = holder.path().join("research-fixture");
        std::fs::create_dir_all(&root).unwrap();
        let readme = "---\n\
stages:\n  states:\n    - name: scoping\n      initial: true\n    - name: review\n    - name: smoke\n    - name: analyze\n    - name: promote\n      terminal: true\n\
---\n\
\n\
# Research workflow\n\
\n\
## Stages\n\
\n\
### `scoping` (lead only, worktree)\n\
scoping-prose-marker\n\
\n\
### `review` (hypothesis only, gate, fresh)\n\
review-prose-marker\n\
\n\
### `smoke` (hypothesis only, worktree)\n\
smoke-prose-marker\n\
\n\
### `analyze` (hypothesis only, fresh, no worktree)\n\
analyze-prose-marker\n\
\n\
### `promote` (hypothesis only, gate, fresh)\n\
promote-prose-marker\n";
        std::fs::write(root.join("README.md"), readme).unwrap();
        // A token entity file so the workflow loads cleanly.
        std::fs::write(
            root.join("task-001.md"),
            "---\nid: 001\ntitle: T0\nstatus: scoping\n---\n\nbody\n",
        )
        .unwrap();

        let mut app = App::load(root).expect("load");
        app.handle_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::NONE));
        assert!(app.is_definition(), "D should switch to Definition mode");

        let mut terminal = Terminal::new(TestBackend::new(160, 80)).expect("terminal");
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .expect("render");
        let rendered = buffer_text(terminal.backend().buffer());

        for stage in ["scoping", "review", "smoke", "analyze", "promote"] {
            let marker = format!("{stage}-prose-marker");
            assert!(
                rendered.contains(&marker),
                "expected prose marker '{marker}' for stage {stage}; rendered=\n{rendered}"
            );
        }
    }
}
