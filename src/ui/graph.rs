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
        WidthTier::Wide => render_wide(
            stages,
            &counts,
            active_stage.as_deref(),
            &glyphs,
            definition,
        ),
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
        lines.extend(padded_feedback_lines(
            arc_line,
            ann_line,
            uniform_width,
            arc_style,
        ));
    }
    if arcs.len() > MAX_FEEDBACK_ROWS {
        let overflow = arcs.len() - MAX_FEEDBACK_ROWS;
        lines.push(Line::from(format!("+{overflow} more feedback edges")));
    }

    lines
}

fn padded_feedback_lines<'a>(
    arc_line: String,
    ann_line: String,
    uniform_width: usize,
    style: Style,
) -> [Line<'a>; 2] {
    [
        padded_styled_line(arc_line, uniform_width, style),
        padded_styled_line(ann_line, uniform_width, style),
    ]
}

fn padded_styled_line<'a>(text: String, uniform_width: usize, style: Style) -> Line<'a> {
    let padding = uniform_width.saturating_sub(visible_width(&text));
    Line::from(vec![
        Span::styled(text, style),
        Span::raw(" ".repeat(padding)),
    ])
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
                arcs.push(FeedbackArc {
                    source_col,
                    target_col,
                });
            }
        }
    }
    arcs
}

/// Returns `(top_line, bottom_line)` for a feedback edge.  The top line
/// carries the vertical markers and the centred `reject` label; the bottom
/// line draws the rounded arc.  Both strings have the same width so the
/// caller can pad them to a uniform width for centred alignment.
fn render_feedback_row(source_col: usize, target_col: usize, g: &GlyphSet) -> (String, String) {
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

/// Build a single ribbon row for a slice of stages (used by render_narrow for 2-row split).
fn build_narrow_row(
    stages: &[StageDefinition],
    counts: &[usize],
    _active: Option<&str>,
    g: &GlyphSet,
) -> String {
    let parts: Vec<String> = stages
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            let count = counts.get(i).copied().unwrap_or(0);
            let node = build_node_text(stage, g);
            format!("{node}({count})")
        })
        .collect();
    parts.join(&format!(" {} ", g.narrow_arrow))
}

fn render_narrow<'a>(
    stages: &'a [StageDefinition],
    counts: &'a [usize],
    active: Option<&str>,
    g: &'a GlyphSet,
) -> Vec<Line<'a>> {
    // AC-5: Split DAG into two rows at the midpoint so it fits at ~80 columns.
    // Row 1: stages[0..mid], Row 2: stages[mid..].
    let mid = stages.len() / 2;
    let (row1_stages, row2_stages) = stages.split_at(mid.max(1).min(stages.len()));
    let row1_counts = &counts[..row1_stages.len().min(counts.len())];
    let row2_counts = if counts.len() > row1_stages.len() {
        &counts[row1_stages.len()..]
    } else {
        &[]
    };

    let row1 = build_narrow_row(row1_stages, row1_counts, active, g);
    let row2 = build_narrow_row(row2_stages, row2_counts, active, g);

    let mut lines = vec![Line::from(row1)];
    if !row2_stages.is_empty() {
        lines.push(Line::from(row2));
    }

    let fb_parts = feedback_annotations(stages, g);
    if !fb_parts.is_empty() {
        lines.push(Line::from(fb_parts.join(", ")));
    }
    lines
}

fn feedback_annotations(stages: &[StageDefinition], g: &GlyphSet) -> Vec<String> {
    stages
        .iter()
        .filter_map(|stage| {
            let target = stage.feedback_to.as_ref()?;
            stages.iter().any(|s| &s.name == target).then(|| {
                format!(
                    "{} rollback on reject: {} {} {}",
                    g.feedback, stage.name, g.narrow_arrow, target
                )
            })
        })
        .collect()
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
mod tests;
