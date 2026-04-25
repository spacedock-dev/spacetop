//! Workflow stage graph rendering.
//!
//! Exposes a single entry point [`render_stage_graph`] that draws the
//! workflow's stage topology (nodes + feedback arcs + counts) inside the
//! Overview's top pane. See `docs/spacetop-dev/add-workflow-graph-view.md`
//! for the locked design.

use ratatui::{
    layout::{Alignment, Rect},
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{OverviewState, ViewScope};
use crate::domain::StageDefinition;
use crate::ui::stage_color;

const ASCII_ENV_VAR: &str = "SPACETOP_ASCII";
const MAX_FEEDBACK_ROWS: usize = 2;

/// Render the workflow stage graph into `area`.
///
/// The renderer is a pure function of the overview state: it reads stages,
/// counts, and the currently selected item, then picks a width tier based on
/// `area.width` and emits the ribbon, counts row, and optional feedback arc
/// row(s).
///
/// **Note (override of task 010):** the `[i/N]` breadcrumb prefix that this
/// renderer used to inject into the block title has been retired — the
/// dedicated tab bar above this pane now carries that information.
pub fn render_stage_graph(frame: &mut Frame<'_>, area: Rect, state: &OverviewState) {
    let ascii = std::env::var(ASCII_ENV_VAR)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let glyphs = glyphs_for(ascii);

    let stages = &state.snapshot().definition.stages;
    let counts = state.stage_counts();
    let counts: Vec<usize> = counts.into_iter().map(|c| c.items).collect();

    let scope_label = match state.view_scope() {
        ViewScope::Active => "active",
        ViewScope::Archived => "archived",
    };
    let archived_label = match state.archived_count() {
        Some(n) => format!("archived: {n}"),
        None => "archived: (press a)".to_string(),
    };
    let workflow_path = state.workflow_dir().display().to_string();

    let active_stage = match state.view_scope() {
        ViewScope::Active => state.selected_item().map(|item| item.status.clone()),
        ViewScope::Archived => None,
    };

    let title = format!(
        "Workflow \u{2014} [{scope_label}] \u{2014} {archived_label} \u{2014} {workflow_path}"
    );

    if stages.is_empty() {
        let paragraph = Paragraph::new(Line::from("(no stages defined)"))
            .block(Block::default().title(title).borders(Borders::ALL))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let inner_width = area.width.saturating_sub(2) as usize;
    let tier = pick_width_tier(inner_width, stages, &counts, &glyphs);

    let lines = match tier {
        WidthTier::Wide => render_wide(stages, &counts, active_stage.as_deref(), &glyphs),
        WidthTier::Narrow => render_narrow(stages, &counts, active_stage.as_deref(), &glyphs),
        WidthTier::VeryNarrow => {
            render_very_narrow(stages, &counts, active_stage.as_deref(), &glyphs)
        }
    };

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title(title).borders(Borders::ALL))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WidthTier {
    Wide,
    Narrow,
    VeryNarrow,
}

#[derive(Debug, Clone)]
struct GlyphSet {
    initial: &'static str,
    terminal: &'static str,
    gate: &'static str,
    worktree: &'static str,
    feedback: &'static str,
    forward_arrow: &'static str,
    narrow_arrow: &'static str,
    arc_down_right: &'static str,
    arc_down_left: &'static str,
    arc_horizontal: &'static str,
}

fn glyphs_for(ascii: bool) -> GlyphSet {
    if ascii {
        GlyphSet {
            initial: ">",
            terminal: "#",
            gate: "!",
            worktree: "@",
            feedback: "<",
            forward_arrow: "->",
            narrow_arrow: "->",
            arc_down_right: "+",
            arc_down_left: "+",
            arc_horizontal: "-",
        }
    } else {
        GlyphSet {
            initial: "\u{25B6}",                       // ▶
            terminal: "\u{25A0}",                      // ■
            gate: "\u{2691}",                          // ⚑
            worktree: "\u{2387}",                      // ⎇
            feedback: "\u{21B6}",                      // ↶
            forward_arrow: "\u{2500}\u{2500}\u{25BA}", // ──►
            narrow_arrow: "\u{2192}",                  // →
            arc_down_right: "\u{2514}",                // └
            arc_down_left: "\u{2518}",                 // ┘
            arc_horizontal: "\u{2500}",                // ─
        }
    }
}

#[derive(Debug, Clone)]
struct ColumnLayout {
    stage_name: String,
    node_text: String,
    /// Byte column where the node text starts in the ribbon line.
    start_col: usize,
    /// Column where the centre of the stage name sits (for counts alignment).
    name_center: usize,
    count: usize,
    is_active: bool,
}

fn build_node_text(stage: &StageDefinition, g: &GlyphSet) -> String {
    // Marker ordering: ⚑ ⎇ ▶ name ■
    let mut leading: Vec<&str> = Vec::new();
    if stage.gate {
        leading.push(g.gate);
    }
    if stage.worktree {
        leading.push(g.worktree);
    }
    if stage.initial {
        leading.push(g.initial);
    }

    let mut parts: Vec<String> = Vec::new();
    if !leading.is_empty() {
        parts.push(leading.join(" "));
    }
    parts.push(stage.name.clone());
    if stage.terminal {
        parts.push(g.terminal.to_string());
    }
    parts.join(" ")
}

fn layout_columns(
    stages: &[StageDefinition],
    counts: &[usize],
    active: Option<&str>,
    g: &GlyphSet,
) -> Vec<ColumnLayout> {
    let separator = format!(" {} ", g.forward_arrow);
    let sep_width = visible_width(&separator);

    let mut out = Vec::with_capacity(stages.len());
    let mut cursor = 0usize;
    for (i, stage) in stages.iter().enumerate() {
        if i > 0 {
            cursor += sep_width;
        }
        let node_text = build_node_text(stage, g);
        let width = visible_width(&node_text);
        let start_col = cursor;
        // Center of the stage name (not markers). Approximate by offsetting from
        // the trailing end: node text is `{leading} {name} {terminal?}`. Name
        // center ≈ node center; alignment target for counts.
        let name_center = start_col + width / 2;
        let count = counts.get(i).copied().unwrap_or(0);
        let is_active = active.map(|s| s == stage.name).unwrap_or(false);
        out.push(ColumnLayout {
            stage_name: stage.name.clone(),
            node_text,
            start_col,
            name_center,
            count,
            is_active,
        });
        cursor += width;
    }
    out
}

fn visible_width(s: &str) -> usize {
    // We only use BMP glyphs; char count is a good proxy (one column per char).
    s.chars().count()
}

fn pick_width_tier(
    inner_width: usize,
    stages: &[StageDefinition],
    counts: &[usize],
    g: &GlyphSet,
) -> WidthTier {
    if inner_width == 0 {
        return WidthTier::VeryNarrow;
    }
    let columns = layout_columns(stages, counts, None, g);
    let wide_width = columns
        .last()
        .map(|c| c.start_col + visible_width(&c.node_text))
        .unwrap_or(0);
    if wide_width <= inner_width {
        return WidthTier::Wide;
    }

    // Narrow form: `name(count) → name(count) → ...`
    let narrow = narrow_summary(stages, counts, g);
    if visible_width(&narrow) <= inner_width {
        return WidthTier::Narrow;
    }
    WidthTier::VeryNarrow
}

fn narrow_summary(stages: &[StageDefinition], counts: &[usize], g: &GlyphSet) -> String {
    // Compact form drops per-stage markers to maximize the chance of fitting
    // on one line. Feedback info is appended separately.
    let parts: Vec<String> = stages
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            let count = counts.get(i).copied().unwrap_or(0);
            format!("{}({count})", stage.name)
        })
        .collect();
    parts.join(&format!(" {} ", g.narrow_arrow))
}

fn render_wide<'a>(
    stages: &'a [StageDefinition],
    counts: &'a [usize],
    active: Option<&str>,
    g: &'a GlyphSet,
) -> Vec<Line<'a>> {
    let cols = layout_columns(stages, counts, active, g);

    // Ribbon line — color each node by its stage; arrows stay neutral.
    let separator = format!(" {} ", g.forward_arrow);
    let mut ribbon_spans: Vec<Span<'a>> = Vec::new();
    for (i, col) in cols.iter().enumerate() {
        if i > 0 {
            ribbon_spans.push(Span::styled(
                separator.clone(),
                Style::default().fg(Color::DarkGray),
            ));
        }
        let mut style = Style::default()
            .fg(stage_color(&col.stage_name))
            .add_modifier(Modifier::BOLD);
        if col.is_active {
            style = style.add_modifier(Modifier::REVERSED);
        }
        ribbon_spans.push(Span::styled(col.node_text.clone(), style));
    }

    // Counts line: place count string centered under each node's name_center.
    let counts_line = build_counts_line(&cols);
    let counts_spans = style_counts_spans(&cols, &counts_line);

    // Feedback arcs: collect (source_col, target_col) pairs whose targets exist.
    let arcs = collect_feedback_arcs(stages, &cols);

    let mut lines: Vec<Line<'a>> = Vec::new();
    lines.push(Line::from(ribbon_spans));
    lines.push(Line::from(counts_spans));

    let max_width = cols
        .last()
        .map(|c| c.start_col + visible_width(&c.node_text))
        .unwrap_or(0);

    let capped: Vec<_> = arcs.iter().take(MAX_FEEDBACK_ROWS).collect();
    for arc in &capped {
        lines.push(Line::from(render_feedback_row(
            arc.source_col,
            arc.target_col,
            max_width,
            &arc.source_stage,
            &arc.target_stage,
            g,
        )));
    }
    if arcs.len() > MAX_FEEDBACK_ROWS {
        let overflow = arcs.len() - MAX_FEEDBACK_ROWS;
        lines.push(Line::from(format!("+{overflow} more feedback edges")));
    }

    lines
}

fn build_counts_line(cols: &[ColumnLayout]) -> String {
    let total_width = cols
        .last()
        .map(|c| c.start_col + visible_width(&c.node_text))
        .unwrap_or(0);
    let mut buf: Vec<char> = vec![' '; total_width];
    for col in cols {
        let s = col.count.to_string();
        let w = s.chars().count();
        let start = col.name_center.saturating_sub(w / 2);
        let end = (start + w).min(buf.len());
        for (i, ch) in s.chars().enumerate() {
            let idx = start + i;
            if idx < end {
                buf[idx] = ch;
            }
        }
    }
    buf.into_iter().collect()
}

fn style_counts_spans<'a>(cols: &[ColumnLayout], counts_line: &str) -> Vec<Span<'a>> {
    // Split counts_line into chunks for each column to style the active one.
    let total: Vec<char> = counts_line.chars().collect();
    let mut spans: Vec<Span<'a>> = Vec::new();
    // Determine active column byte ranges by name_center ± half-count width.
    let mut regions: Vec<(usize, usize, bool)> = Vec::new();
    let mut cursor = 0usize;
    for col in cols {
        let s = col.count.to_string();
        let w = s.chars().count();
        let start = col.name_center.saturating_sub(w / 2);
        let end = (start + w).min(total.len());
        if start > cursor {
            regions.push((cursor, start, false));
        }
        regions.push((start, end, col.is_active));
        cursor = end;
    }
    if cursor < total.len() {
        regions.push((cursor, total.len(), false));
    }
    for (start, end, active) in regions {
        let text: String = total[start..end].iter().collect();
        if active {
            spans.push(Span::styled(
                text,
                Style::default().add_modifier(Modifier::REVERSED),
            ));
        } else {
            spans.push(Span::raw(text));
        }
    }
    spans
}

#[derive(Debug)]
struct FeedbackArc {
    source_col: usize,
    target_col: usize,
    source_stage: String,
    target_stage: String,
}

fn collect_feedback_arcs(stages: &[StageDefinition], cols: &[ColumnLayout]) -> Vec<FeedbackArc> {
    let mut arcs = Vec::new();
    for (i, stage) in stages.iter().enumerate() {
        if let Some(target) = &stage.feedback_to {
            let target_idx = stages.iter().position(|s| &s.name == target);
            if let Some(t) = target_idx {
                let source_col = cols[i].name_center;
                let target_col = cols[t].name_center;
                arcs.push(FeedbackArc {
                    source_col,
                    target_col,
                    source_stage: stage.name.clone(),
                    target_stage: stages[t].name.clone(),
                });
            }
        }
    }
    arcs
}

fn render_feedback_row(
    source_col: usize,
    target_col: usize,
    total_width: usize,
    source_stage: &str,
    target_stage: &str,
    g: &GlyphSet,
) -> String {
    let annotation = format!(
        " {} {} {} {}",
        g.feedback, source_stage, g.narrow_arrow, target_stage
    );
    let ann_w = visible_width(&annotation);
    let min_row = total_width.max(source_col + 1).max(target_col + 1) + ann_w + 1;
    let mut buf: Vec<String> = (0..min_row).map(|_| " ".to_string()).collect();

    let (left, right) = if source_col <= target_col {
        (source_col, target_col)
    } else {
        (target_col, source_col)
    };
    if right > left + 1 {
        for cell in buf.iter_mut().take(right).skip(left + 1) {
            *cell = g.arc_horizontal.to_string();
        }
    }
    if left < buf.len() {
        buf[left] = g.arc_down_right.to_string();
    }
    if right < buf.len() {
        buf[right] = g.arc_down_left.to_string();
    }
    let ann_start = buf.len().saturating_sub(ann_w);
    for (i, ch) in annotation.chars().enumerate() {
        let idx = ann_start + i;
        if idx < buf.len() {
            buf[idx] = ch.to_string();
        }
    }
    buf.join("")
}

fn render_narrow<'a>(
    stages: &'a [StageDefinition],
    counts: &'a [usize],
    _active: Option<&str>,
    g: &'a GlyphSet,
) -> Vec<Line<'a>> {
    let mut lines = vec![Line::from(narrow_summary(stages, counts, g))];
    // Append textual feedback annotation line if any valid feedback edges.
    let mut fb_parts: Vec<String> = Vec::new();
    for stage in stages.iter() {
        if let Some(target) = &stage.feedback_to {
            if stages.iter().any(|s| &s.name == target) {
                fb_parts.push(format!(
                    "{} rollback on reject: {} {} {}",
                    g.feedback, stage.name, g.narrow_arrow, target
                ));
            }
        }
    }
    if !fb_parts.is_empty() {
        lines.push(Line::from(fb_parts.join(", ")));
    }
    lines
}

fn render_very_narrow<'a>(
    stages: &'a [StageDefinition],
    counts: &'a [usize],
    active: Option<&str>,
    g: &'a GlyphSet,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::new();
    for (i, stage) in stages.iter().enumerate() {
        let count = counts.get(i).copied().unwrap_or(0);
        let mut marker = String::new();
        if stage.gate {
            marker.push_str(g.gate);
        }
        if stage.worktree {
            marker.push_str(g.worktree);
        }
        if stage.initial {
            marker.push_str(g.initial);
        }
        if stage.terminal {
            marker.push_str(g.terminal);
        }
        let marker_prefix = if marker.is_empty() {
            String::new()
        } else {
            format!("{marker} ")
        };
        let text = format!("{marker_prefix}{} ({count})", stage.name);
        let is_active = active.map(|s| s == stage.name).unwrap_or(false);
        if is_active {
            lines.push(Line::from(Span::styled(
                text,
                Style::default().add_modifier(Modifier::REVERSED),
            )));
        } else {
            lines.push(Line::from(text));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, layout::Rect, Terminal};
    use std::path::PathBuf;
    use std::sync::Mutex;

    use crate::app::{App, OverviewState};
    use crate::domain::{StageDefinition, WorkItem, WorkflowDefinition, WorkflowSnapshot};

    // Serialize tests that touch SPACETOP_ASCII env var.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("")
    }

    fn real_workflow() -> App {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        App::load(root).expect("real workflow should load")
    }

    fn workflow_with_active_item() -> App {
        let root = PathBuf::from("/tmp/spacetop-graph");
        let snapshot = WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: root.clone(),
                stages: vec![
                    stage("design", true, false, false, false, None),
                    stage("plan", false, false, false, false, None),
                    stage("done", false, true, false, false, None),
                ],
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
            },
            items: vec![make_item("001", "plan", "Plan task")],
        };
        App::from_snapshot(root, snapshot)
    }

    fn state_of(app: &App) -> &OverviewState {
        app.as_overview().expect("overview mode")
    }

    fn render_to_string(app: &App, width: u16, height: u16) -> String {
        let mut terminal =
            Terminal::new(TestBackend::new(width, height)).expect("create test terminal");
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, width, height);
                render_stage_graph(frame, area, state_of(app));
            })
            .expect("draw");
        buffer_text(terminal.backend().buffer())
    }

    fn monotonic_contains(haystack: &str, needles: &[&str]) -> bool {
        let mut cursor = 0usize;
        for n in needles {
            match haystack[cursor..].find(n) {
                Some(pos) => cursor += pos + n.len(),
                None => return false,
            }
        }
        true
    }

    #[test]
    fn glyphs_for_respects_ascii_flag() {
        let u = glyphs_for(false);
        let a = glyphs_for(true);
        assert_eq!(u.initial, "\u{25B6}");
        assert_eq!(a.initial, ">");
        assert_ne!(u.forward_arrow, a.forward_arrow);
    }

    #[test]
    fn layout_columns_places_initial_marker_on_first_stage() {
        let g = glyphs_for(false);
        let stages = vec![stage("design", true, false, false, false, None)];
        let cols = layout_columns(&stages, &[0], None, &g);
        assert!(cols[0].node_text.starts_with("\u{25B6}"));
    }

    #[test]
    fn layout_columns_places_terminal_marker_on_last_stage() {
        let g = glyphs_for(false);
        let stages = vec![stage("done", false, true, false, false, None)];
        let cols = layout_columns(&stages, &[0], None, &g);
        assert!(cols[0].node_text.ends_with("\u{25A0}"));
    }

    #[test]
    fn layout_columns_marker_ordering_is_gate_worktree_initial_name_terminal() {
        let g = glyphs_for(false);
        let stages = vec![stage("x", true, true, true, true, None)];
        let cols = layout_columns(&stages, &[0], None, &g);
        // Expect ⚑ first, then ⎇, then ▶, then space+name, then space+■.
        let t = &cols[0].node_text;
        let gate = t.find('\u{2691}').unwrap();
        let wt = t.find('\u{2387}').unwrap();
        let ini = t.find('\u{25B6}').unwrap();
        let nm = t.find('x').unwrap();
        let term = t.find('\u{25A0}').unwrap();
        assert!(gate < wt && wt < ini && ini < nm && nm < term);
    }

    #[test]
    fn pick_width_tier_returns_expected_tier_for_sample_widths() {
        let app = real_workflow();
        let stages = &app.snapshot().definition.stages;
        let counts: Vec<usize> = app.stage_counts().into_iter().map(|c| c.items).collect();
        let g = glyphs_for(false);
        assert_eq!(
            pick_width_tier(120, stages, &counts, &g),
            WidthTier::Wide,
            "120 cols should fit wide"
        );
        // 38 inner cols is too small for wide but fits narrow compact form for our 5-stage set.
        let narrow = pick_width_tier(38, stages, &counts, &g);
        assert!(
            matches!(narrow, WidthTier::Narrow | WidthTier::VeryNarrow),
            "small width should not be Wide"
        );
        assert_eq!(
            pick_width_tier(22, stages, &counts, &g),
            WidthTier::VeryNarrow
        );
    }

    #[test]
    fn renders_wide_ribbon_with_unicode_glyphs_for_real_workflow() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let app = real_workflow();
        let rendered = render_to_string(&app, 120, 10);
        for name in ["design", "plan", "implement", "review", "done"] {
            assert!(rendered.contains(name), "missing stage {name}");
        }
        assert!(monotonic_contains(
            &rendered,
            &["design", "plan", "implement", "review", "done"]
        ));
        // Markers.
        assert!(rendered.contains("\u{25B6}"), "missing initial marker");
        assert!(rendered.contains("\u{25A0}"), "missing terminal marker");
        assert!(rendered.contains("\u{2691}"), "missing gate marker");
        assert!(rendered.contains("\u{2387}"), "missing worktree marker");
    }

    #[test]
    fn renders_rollback_annotation_for_review_feedback_path() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let app = real_workflow();
        let rendered = render_to_string(&app, 120, 12);
        assert!(
            rendered.contains("\u{2514}") && rendered.contains("\u{2518}"),
            "missing arc corner glyphs (└ ┘)"
        );
        assert!(
            rendered.contains("\u{21B6} review \u{2192} implement"),
            "missing rollback annotation with stage names"
        );
        assert!(
            !rendered.contains("feedback-to"),
            "workflow graph should not expose raw workflow schema terminology"
        );
    }

    #[test]
    fn reflects_different_workflow_topology() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let snapshot = WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: PathBuf::from("/tmp/other"),
                stages: vec![
                    stage("alpha", true, false, false, false, None),
                    stage("beta", false, false, false, false, None),
                    stage("gamma", false, true, false, false, None),
                ],
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
            },
            items: Vec::new(),
        };
        let app = App::from_snapshot(PathBuf::from("/tmp/other"), snapshot);
        let rendered = render_to_string(&app, 120, 10);
        assert!(rendered.contains("alpha"));
        assert!(rendered.contains("beta"));
        assert!(rendered.contains("gamma"));
        assert!(!rendered.contains("design"));
        assert!(!rendered.contains("review"));
        assert!(!rendered.contains("\u{21B6}"), "no feedback glyph for topology without feedback edges");
    }

    #[test]
    fn narrow_tier_renders_compact_textual_summary() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let app = real_workflow();
        // Width chosen so the wide ribbon doesn't fit but the compact narrow
        // form does — see pick_width_tier().
        let rendered = render_to_string(&app, 58, 10);
        for name in ["design", "plan", "implement", "review", "done"] {
            assert!(rendered.contains(name), "missing stage {name}");
        }
        assert!(
            rendered.contains('\u{2192}') || rendered.contains("->"),
            "missing narrow arrow"
        );
    }

    #[test]
    fn very_narrow_tier_stacks_one_stage_per_line() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let app = real_workflow();
        let width: u16 = 24;
        let height: u16 = 12;
        let rendered = render_to_string(&app, width, height);
        // Verify each stage name appears on a distinct row (strictly increasing row index).
        let cols = width as usize;
        let mut last_row: Option<usize> = None;
        for name in ["design", "plan", "implement", "review", "done"] {
            let pos = rendered
                .find(name)
                .unwrap_or_else(|| panic!("missing stage {name}"));
            let row = pos / cols;
            if let Some(prev) = last_row {
                assert!(row > prev, "stage {name} at row {row} not after {prev}");
            }
            last_row = Some(row);
        }
    }

    #[test]
    fn counts_row_aligns_under_nodes_and_marks_active_stage() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let app = workflow_with_active_item();
        let width: u16 = 120;
        let height: u16 = 10;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| {
                render_stage_graph(frame, Rect::new(0, 0, width, height), state_of(&app));
            })
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let text = buffer_text(&buffer);
        // Counts row exists and contains every count number from stage_counts.
        for c in app.stage_counts() {
            assert!(
                text.contains(&c.items.to_string()),
                "missing count {} for {}",
                c.items,
                c.name
            );
        }
        // Find at least one reversed-style cell — corresponds to the active stage's count cell.
        let mut has_reversed = false;
        for y in 0..height {
            for x in 0..width {
                let cell = &buffer[(x, y)];
                if cell.style().add_modifier.contains(Modifier::REVERSED) {
                    has_reversed = true;
                    break;
                }
            }
            if has_reversed {
                break;
            }
        }
        assert!(
            has_reversed,
            "expected at least one reversed cell for the active stage"
        );
    }

    #[test]
    fn header_row_contains_scope_label_and_workflow_path() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let app = real_workflow();
        // Render at a width comfortably wider than the workflow path so the
        // header doesn't get truncated by the block title; otherwise this
        // assertion is really testing terminal width, not header content.
        let path_len = app.workflow_dir().display().to_string().chars().count() as u16;
        // Reserve generous slack for the block borders and the title prefix
        // ("Workflow — [active] — archived: ... — ") so the header doesn't get
        // truncated by the block title renderer.
        let width = path_len.saturating_add(80).max(200);
        let rendered = render_to_string(&app, width, 10);
        assert!(rendered.contains("active"), "missing scope label");
        let p = app.workflow_dir().display().to_string();
        // Path is derived from the snapshot's workflow_dir — check the last
        // path component, not a hard-coded fixture name.
        let last = std::path::Path::new(&p)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&p);
        assert!(
            rendered.contains(last),
            "missing workflow path component {last}"
        );
    }

    #[test]
    fn ascii_fallback_swaps_glyphs_when_env_set() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var(ASCII_ENV_VAR, "1");
        let app = real_workflow();
        let rendered = render_to_string(&app, 160, 10);
        std::env::remove_var(ASCII_ENV_VAR);

        assert!(rendered.contains('>'), "missing ASCII initial '>'");
        assert!(rendered.contains('#'), "missing ASCII terminal '#'");
        assert!(rendered.contains('!'), "missing ASCII gate '!'");
        assert!(rendered.contains('@'), "missing ASCII worktree '@'");
        assert!(rendered.contains('<'), "missing ASCII feedback '<'");
        assert!(rendered.contains("->"), "missing ASCII forward arrow");
        assert!(!rendered.contains('\u{25B6}'), "Unicode initial leaked");
        assert!(!rendered.contains('\u{25A0}'), "Unicode terminal leaked");
        assert!(!rendered.contains('\u{2691}'), "Unicode gate leaked");
        assert!(!rendered.contains('\u{2387}'), "Unicode worktree leaked");
        assert!(!rendered.contains('\u{21B6}'), "Unicode feedback leaked");
    }

    #[test]
    fn module_surface_is_minimal() {
        let _f: fn(&mut ratatui::prelude::Frame<'_>, Rect, &OverviewState) = render_stage_graph;
    }

    #[test]
    fn no_breadcrumb_in_graph_header() {
        // The breadcrumb prefix that task 010 added to the graph block title
        // has been retired (the tab bar above the graph carries that info
        // now). This test locks the absence of the [i/N] prefix in any
        // session.
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let app = real_workflow();
        let path_len = app.workflow_dir().display().to_string().chars().count() as u16;
        let width = path_len.saturating_add(80).max(200);
        let height: u16 = 10;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| {
                render_stage_graph(frame, Rect::new(0, 0, width, height), state_of(&app));
            })
            .expect("draw");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            !rendered.contains("[1/1]") && !rendered.contains("[2/3]"),
            "graph header must not contain a [i/N] breadcrumb prefix"
        );
    }

    // --- helpers ---

    fn stage(
        name: &str,
        initial: bool,
        terminal: bool,
        gate: bool,
        worktree: bool,
        feedback_to: Option<&str>,
    ) -> StageDefinition {
        StageDefinition {
            name: name.to_string(),
            initial,
            terminal,
            gate,
            fresh: false,
            feedback_to: feedback_to.map(|s| s.to_string()),
            worktree,
            concurrency: None,
        }
    }

    #[allow(dead_code)]
    fn make_item(id: &str, status: &str, title: &str) -> WorkItem {
        WorkItem {
            path: PathBuf::from(format!("{id}.md")),
            id: id.to_string(),
            title: title.to_string(),
            status: status.to_string(),
            source: None,
            started: None,
            completed: None,
            verdict: None,
            score: None,
            worktree: None,
            issue: None,
            pr: None,
            body: String::new(),
        }
    }
}
