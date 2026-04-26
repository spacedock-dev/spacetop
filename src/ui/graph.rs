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
use crate::domain::{StageDefinition, WorkflowDefinition};

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
    let active_stage = match state.view_scope() {
        ViewScope::Active => state.selected_item().map(|item| item.status.clone()),
        ViewScope::Archived => None,
    };

    let title = format!("Workflow \u{2014} [{scope_label}] \u{2014} {archived_label}");

    if stages.is_empty() {
        let paragraph = Paragraph::new(Line::from("(no stages defined)"))
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::TOP | Borders::BOTTOM),
            )
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    // With TOP+BOTTOM borders only, the inner width equals the full area width
    // (no left/right border columns are consumed).
    let inner_width = area.width as usize;
    let tier = pick_width_tier(inner_width, stages, &counts, &glyphs);

    let definition = &state.snapshot().definition;
    let lines = match tier {
        WidthTier::Wide => render_wide(stages, &counts, active_stage.as_deref(), &glyphs, definition),
        WidthTier::Narrow => render_narrow(stages, &counts, active_stage.as_deref(), &glyphs),
        WidthTier::VeryNarrow => {
            render_very_narrow(stages, &counts, active_stage.as_deref(), &glyphs)
        }
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::TOP | Borders::BOTTOM),
        )
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
    arc_horizontal: &'static str,
    arc_vert: &'static str,
    arc_up_arrow: &'static str,
    arc_corner_up_right: &'static str,
    arc_corner_up_left: &'static str,
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
            arc_horizontal: "-",
            arc_vert: "|",
            arc_up_arrow: "^",
            arc_corner_up_right: "\\",
            arc_corner_up_left: "/",
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
            arc_horizontal: "\u{2500}",                // ─
            arc_vert: "\u{2502}",                      // │
            arc_up_arrow: "\u{2191}",                  // ↑
            arc_corner_up_right: "\u{2570}",           // ╰
            arc_corner_up_left: "\u{256F}",            // ╯
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
    // Exactly one leading glyph per stage, resolved by role priority:
    //   initial → ▶, gate → ⚑, worktree/branchable → ⎇, else none.
    // The terminal suffix (■) is appended separately.
    let leading: Option<&str> = if stage.initial {
        Some(g.initial)
    } else if stage.gate {
        Some(g.gate)
    } else if stage.worktree {
        Some(g.worktree)
    } else {
        None
    };

    let mut parts: Vec<String> = Vec::new();
    if let Some(glyph) = leading {
        parts.push(glyph.to_string());
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
    definition: &WorkflowDefinition,
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
            .fg(definition.stage_color_for(&col.stage_name))
            .add_modifier(Modifier::BOLD);
        if col.is_active {
            style = style.add_modifier(Modifier::REVERSED);
        }
        ribbon_spans.push(Span::styled(col.node_text.clone(), style));
    }

    let max_width = cols
        .last()
        .map(|c| c.start_col + visible_width(&c.node_text))
        .unwrap_or(0);

    // Counts line: place count string centered under each node's name_center.
    let counts_line = build_counts_line(&cols);

    // Feedback arcs.
    let arcs = collect_feedback_arcs(stages, &cols);
    let capped: Vec<_> = arcs.iter().take(MAX_FEEDBACK_ROWS).collect();
    let arc_pairs: Vec<(String, String)> = capped
        .iter()
        .map(|arc| render_feedback_row(arc.source_col, arc.target_col, g))
        .collect();

    // All lines must share the same width so the Paragraph's centred alignment
    // keeps ribbon, counts, arc, and annotation rows aligned with each other.
    let uniform_width = arc_pairs
        .iter()
        .map(|(arc, ann)| visible_width(arc).max(visible_width(ann)))
        .max()
        .unwrap_or(0)
        .max(max_width);

    let pad_len = uniform_width.saturating_sub(max_width);
    if pad_len > 0 {
        ribbon_spans.push(Span::raw(" ".repeat(pad_len)));
    }
    let padded_counts = format!("{counts_line}{}", " ".repeat(pad_len));
    let counts_spans = style_counts_spans(&cols, &padded_counts);

    let mut lines: Vec<Line<'a>> = Vec::new();
    lines.push(Line::from(ribbon_spans));
    lines.push(Line::from(counts_spans));

    let arc_style = Style::default().fg(Color::Red);
    for (arc_line, ann_line) in arc_pairs {
        let arc_pad = uniform_width.saturating_sub(visible_width(&arc_line));
        let ann_pad = uniform_width.saturating_sub(visible_width(&ann_line));
        lines.push(Line::from(vec![
            Span::styled(arc_line, arc_style),
            Span::raw(" ".repeat(arc_pad)),
        ]));
        lines.push(Line::from(vec![
            Span::styled(ann_line, arc_style),
            Span::raw(" ".repeat(ann_pad)),
        ]));
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
            spans.push(Span::styled(
                text,
                Style::default().add_modifier(Modifier::DIM),
            ));
        }
    }
    spans
}

#[derive(Debug)]
struct FeedbackArc {
    source_col: usize,
    target_col: usize,
}

fn collect_feedback_arcs(stages: &[StageDefinition], cols: &[ColumnLayout]) -> Vec<FeedbackArc> {
    let mut arcs = Vec::new();
    for (i, stage) in stages.iter().enumerate() {
        if let Some(target) = &stage.feedback_to {
            let target_idx = stages.iter().position(|s| &s.name == target);
            if let Some(t) = target_idx {
                let source_col = cols[i].name_center;
                let target_col = cols[t].name_center;
                arcs.push(FeedbackArc { source_col, target_col });
            }
        }
    }
    arcs
}

/// Returns `(top_line, bottom_line)` for a feedback edge.  The top line
/// carries the vertical markers and the centred `reject` label; the bottom
/// line draws the rounded arc.  Both strings have the same width so the
/// caller can pad them to a uniform width for centred alignment.
fn render_feedback_row(
    source_col: usize,
    target_col: usize,
    g: &GlyphSet,
) -> (String, String) {
    // Two-line feedback rendering:
    //   line 1:  ↑   reject   │     (↑ at target, │ at source, label centred)
    //   line 2:  ╰────────────╯     (rounded corners with horizontal fill)
    let (left, right, target_is_left) = if source_col > target_col {
        (target_col, source_col, true)
    } else {
        (source_col, target_col, false)
    };

    let label = "reject";
    let label_w = visible_width(label);
    let total_w = right + 1;

    // Line 1: vertical markers at both columns + centred label.
    let mut top: Vec<String> = vec![" ".to_string(); total_w];
    if target_is_left {
        top[left] = g.arc_up_arrow.to_string();
        top[right] = g.arc_vert.to_string();
    } else {
        top[left] = g.arc_vert.to_string();
        top[right] = g.arc_up_arrow.to_string();
    }
    if right > left + 1 + label_w {
        let arc_center = (left + right) / 2;
        let label_start = arc_center.saturating_sub(label_w / 2);
        for (i, ch) in label.chars().enumerate() {
            if let Some(cell) = top.get_mut(label_start + i) {
                *cell = ch.to_string();
            }
        }
    }

    // Line 2: rounded corners + horizontal fill.
    let mut bottom: Vec<String> = vec![" ".to_string(); total_w];
    if right > left + 1 {
        for cell in bottom.iter_mut().take(right).skip(left + 1) {
            *cell = g.arc_horizontal.to_string();
        }
    }
    bottom[left] = g.arc_corner_up_right.to_string();
    bottom[right] = g.arc_corner_up_left.to_string();

    (top.join(""), bottom.join(""))
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
                stage_colors: std::collections::HashMap::new(),
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
    fn layout_columns_single_glyph_per_stage_initial_takes_priority() {
        // Spec: exactly one leading glyph per stage, priority: initial > gate > worktree.
        // A stage that is initial, gate, and worktree should display only ▶.
        let g = glyphs_for(false);
        let stages = vec![stage("x", true, true, true, true, None)];
        let cols = layout_columns(&stages, &[0], None, &g);
        let t = &cols[0].node_text;
        // Must contain initial glyph ▶, name 'x', and terminal ■.
        assert!(t.contains('\u{25B6}'), "missing initial glyph ▶");
        assert!(t.contains('x'), "missing stage name");
        assert!(t.contains('\u{25A0}'), "missing terminal glyph ■");
        // Must NOT contain gate ⚑ or worktree ⎇ (single-glyph rule).
        assert!(!t.contains('\u{2691}'), "gate glyph ⚑ must not appear when initial is set");
        assert!(!t.contains('\u{2387}'), "worktree glyph ⎇ must not appear when initial is set");
        // ▶ must precede 'x' which must precede ■.
        let ini = t.find('\u{25B6}').unwrap();
        let nm = t.find('x').unwrap();
        let term = t.find('\u{25A0}').unwrap();
        assert!(ini < nm && nm < term);
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
            rendered.contains("\u{2570}") && rendered.contains("\u{256F}"),
            "missing arc corner glyphs (╰ ╯)"
        );
        assert!(
            rendered.contains("\u{2191}"),
            "missing upward arrow at target column"
        );
        assert!(
            rendered.contains("\u{2502}"),
            "missing vertical bar at source column"
        );
        assert!(rendered.contains("reject"), "missing 'reject' label");
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
                stage_colors: std::collections::HashMap::new(),
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
        assert!(!rendered.contains("\u{2570}"), "no arc corner for topology without feedback edges");
        assert!(!rendered.contains("reject"), "no reject label for topology without feedback edges");
    }

    #[test]
    fn narrow_tier_renders_compact_textual_summary() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let app = real_workflow();
        // Width chosen so the wide ribbon doesn't fit but the compact narrow
        // form does — see pick_width_tier(). With TOP+BOTTOM-only borders,
        // inner_width equals area.width directly (no left/right columns
        // consumed), so the threshold dropped by 2 vs. the old ALL-border code.
        let rendered = render_to_string(&app, 56, 10);
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
    fn header_row_contains_scope_label_and_archived_count_only() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let app = real_workflow();
        let width: u16 = 160;
        let rendered = render_to_string(&app, width, 10);
        assert!(rendered.contains("active"), "missing scope label");
        assert!(
            rendered.contains("archived:"),
            "missing archived count/status component"
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
        assert!(rendered.contains('^'), "missing ASCII arc up-arrow '^'");
        assert!(rendered.contains('|'), "missing ASCII arc vertical '|'");
        assert!(rendered.contains("->"), "missing ASCII forward arrow");
        assert!(!rendered.contains('\u{25B6}'), "Unicode initial leaked");
        assert!(!rendered.contains('\u{25A0}'), "Unicode terminal leaked");
        assert!(!rendered.contains('\u{2691}'), "Unicode gate leaked");
        assert!(!rendered.contains('\u{2387}'), "Unicode worktree leaked");
        assert!(!rendered.contains('\u{2191}'), "Unicode arc up-arrow leaked");
        assert!(!rendered.contains('\u{2570}'), "Unicode arc corner leaked");
        // Note: ratatui's block borders use │ and ─ regardless of mode, so we
        // don't assert their absence here.
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

    #[test]
    fn dag_single_glyph_per_stage() {
        // Spec: no stage node text should contain two consecutive vocabulary glyphs.
        let g = glyphs_for(false);
        let vocab: &[char] = &['\u{25B6}', '\u{2387}', '\u{2691}', '\u{25A0}'];
        // Build stages covering all roles.
        let stages = vec![
            stage("start", true, false, false, false, None),  // initial → ▶
            stage("check", false, false, true, false, None),   // gate → ⚑
            stage("work", false, false, false, true, None),    // worktree → ⎇
            stage("done", false, true, false, false, None),    // terminal → ■ suffix
        ];
        for s in &stages {
            let text = build_node_text(s, &g);
            // Find any two adjacent chars that are both in vocab.
            let chars: Vec<char> = text.chars().collect();
            for i in 0..chars.len().saturating_sub(1) {
                assert!(
                    !(vocab.contains(&chars[i]) && vocab.contains(&chars[i + 1])),
                    "stage '{}' node text {:?} has two consecutive vocabulary glyphs at positions {i}/{}",
                    s.name, text, i + 1
                );
            }
        }
    }

    #[test]
    fn rollback_arc_is_red() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let app = real_workflow();
        let width: u16 = 160;
        let height: u16 = 12;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| {
                render_stage_graph(frame, Rect::new(0, 0, width, height), state_of(&app));
            })
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        // Find cells containing arc corner chars ╰ or ╯ and verify they are red.
        let arc_left = '\u{2570}';  // ╰
        let arc_right = '\u{256F}'; // ╯
        let mut found_arc = false;
        for y in 0..height {
            for x in 0..width {
                let cell = &buffer[(x, y)];
                let symbol = cell.symbol();
                let ch = symbol.chars().next().unwrap_or(' ');
                if ch == arc_left || ch == arc_right {
                    found_arc = true;
                    assert_eq!(
                        cell.style().fg,
                        Some(Color::Red),
                        "arc char {:?} at ({x},{y}) must be red, got {:?}",
                        ch,
                        cell.style().fg
                    );
                }
            }
        }
        assert!(found_arc, "expected to find arc corner chars ╰/╯ in the rendered output");
    }

    #[test]
    fn dag_oklch_colors_are_rgb() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(ASCII_ENV_VAR);
        let app = real_workflow();
        let width: u16 = 160;
        let height: u16 = 10;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| {
                render_stage_graph(frame, Rect::new(0, 0, width, height), state_of(&app));
            })
            .expect("draw");
        let buffer = terminal.backend().buffer();
        // At least 3 Color::Rgb fg colors must appear in the rendered graph.
        let rgb_count = (0..height)
            .flat_map(|y| (0..width).map(move |x| buffer[(x, y)].style().fg))
            .flatten()
            .filter(|c| matches!(c, Color::Rgb(_, _, _)))
            .count();
        assert!(rgb_count >= 3, "expected at least 3 Rgb-colored cells in DAG, found {rgb_count}");
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
