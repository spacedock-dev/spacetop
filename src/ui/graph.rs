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
/// Number of blank padding lines inserted between consecutive stage rows in
/// the wrapped Narrow tier and the VeryNarrow multi-column grid (cycle 3
/// captain feedback; tightened from 3 to 1 in cycle 4). Pure spacers — no
/// glyph.
const INTER_ROW_PADDING_LINES: usize = 1;

/// Compute the usable horizontal budget for the stage graph: roughly 90% of
/// the inner pane width, with a floor that lets very narrow panes still
/// consume what they can (cycle 3 captain feedback). The remaining 10%
/// becomes a left/right margin around the rendered graph.
fn usable_inner_width(inner_width: usize) -> usize {
    if inner_width < 20 {
        // For tiny panes (<20 cols), do not steal columns for a margin —
        // every cell already counts. Returns inner_width so margins are 0.
        inner_width
    } else {
        // inner_width * 9 / 10, with a hard guarantee that the budget is at
        // least 1 col less than inner_width so margins are non-zero on
        // realistic pane sizes (inner_width >= 20 => usable <= 18).
        let budget = inner_width.saturating_mul(9) / 10;
        budget.min(inner_width.saturating_sub(2)).max(1)
    }
}

/// Returns `(left, right)` horizontal margin widths so that
/// `left + usable_width + right == inner_width`. The slack splits evenly,
/// with any odd extra column going to the right margin.
fn horizontal_margins(inner_width: usize, usable_width: usize) -> (usize, usize) {
    let slack = inner_width.saturating_sub(usable_width);
    let left = slack / 2;
    let right = slack - left;
    (left, right)
}

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
    // (no left/right border columns are consumed). The inner height is the area
    // height minus the two horizontal borders.
    let inner_width = area.width as usize;
    let inner_height = (area.height as usize).saturating_sub(2).max(1);
    // Cycle 3: the wrapping tiers render into roughly 90% of inner_width with
    // the remaining 10% used as left+right margin. Thread the same budget
    // through `pick_width_tier` so the tier decision and the renderer agree.
    let usable_width = usable_inner_width(inner_width);
    let tier = pick_width_tier(usable_width, stages, &counts, &glyphs);

    let definition = &state.snapshot().definition;
    let lines = match tier {
        WidthTier::Wide => render_dag(
            stages,
            &counts,
            active_stage.as_deref(),
            &glyphs,
            inner_width,
            definition,
        ),
        WidthTier::Narrow => render_narrow(
            stages,
            &counts,
            active_stage.as_deref(),
            &glyphs,
            inner_width,
            definition,
        ),
        WidthTier::VeryNarrow => render_very_narrow(
            stages,
            &counts,
            active_stage.as_deref(),
            &glyphs,
            inner_width,
            inner_height,
            definition,
        ),
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
    /// Column where the centre of the stage name sits (used by feedback-arc
    /// geometry to anchor `╰`/`╯` corners and `↑` arrowheads).
    name_center: usize,
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
    // DAG tier: inline `name(count)` nodes connected by forward arrows must
    // fit on a single row within the usable inner-width budget.
    let columns = dag_layout_columns(stages, counts, None, g);
    let dag_width = columns
        .last()
        .map(|c| c.start_col + visible_width(&c.node_text))
        .unwrap_or(0);
    if dag_width <= inner_width {
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

/// Build the inline DAG node text for a stage: `{leading?} {name}({count}){terminal?}`.
///
/// The DAG tier (entity 010) collapses the previous separate "counts row"
/// into the node text so each stage reads as a single self-contained
/// `name(count)` node connected by drawn `──▶` edges. Markers (initial /
/// gate / worktree / terminal) keep the same single-glyph-per-stage rule
/// from `build_node_text` so the existing marker tests still hold.
fn build_dag_node_text(stage: &StageDefinition, count: usize, g: &GlyphSet) -> String {
    let leading: Option<&str> = if stage.initial {
        Some(g.initial)
    } else if stage.gate {
        Some(g.gate)
    } else if stage.worktree {
        Some(g.worktree)
    } else {
        None
    };
    let mut text = String::new();
    if let Some(glyph) = leading {
        text.push_str(glyph);
        text.push(' ');
    }
    text.push_str(&stage.name);
    text.push('(');
    text.push_str(&count.to_string());
    text.push(')');
    if stage.terminal {
        text.push(' ');
        text.push_str(g.terminal);
    }
    text
}

/// Layout columns for the DAG tier. Identical in shape to `layout_columns`
/// but uses `build_dag_node_text` so the per-column width accounts for the
/// inline `(count)` suffix.
fn dag_layout_columns(
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
        let count = counts.get(i).copied().unwrap_or(0);
        let node_text = build_dag_node_text(stage, count, g);
        let width = visible_width(&node_text);
        let start_col = cursor;
        let name_center = start_col + width / 2;
        let is_active = active.map(|s| s == stage.name).unwrap_or(false);
        out.push(ColumnLayout {
            stage_name: stage.name.clone(),
            node_text,
            start_col,
            name_center,
            is_active,
        });
        cursor += width;
    }
    out
}

/// Render the workflow as a single-row ASCII DAG with line-drawing edges
/// between stages and feedback arcs drawn UNDER the chain (entity 010).
///
/// Layout:
///   line 1: ` {leading}? name(count) {terminal}? ` ── `──▶` ── ... (the
///           forward chain, with inline counts collapsed into the node text)
///   lines 2..N: paired (label, arc) rows for each `feedback-to:` edge
///
/// Each stage span carries `stage_color_for(name)` + BOLD; the active stage
/// also carries REVERSED. Feedback arcs render red.
///
/// All lines are right-padded to a uniform width that matches the inner
/// pane width so the outer `Paragraph::alignment(Center)` is a no-op (the
/// graph hugs the left margin instead of drifting on wide panes).
fn render_dag<'a>(
    stages: &'a [StageDefinition],
    counts: &'a [usize],
    active: Option<&str>,
    g: &'a GlyphSet,
    inner_width: usize,
    definition: &WorkflowDefinition,
) -> Vec<Line<'a>> {
    let cols = dag_layout_columns(stages, counts, active, g);

    // The DAG occupies the leftmost `usable_width` cells of each row so the
    // centered Paragraph + the 10% right-margin contract stay aligned with
    // the wrapped tiers (cycle 3 from entity 009).
    let usable_width = usable_inner_width(inner_width);
    let (left_margin, right_margin) = horizontal_margins(inner_width, usable_width);

    // Chain line — color each node by its stage; arrows stay neutral.
    let separator = format!(" {} ", g.forward_arrow);
    let mut chain_spans: Vec<Span<'a>> = Vec::new();
    if left_margin > 0 {
        chain_spans.push(Span::raw(" ".repeat(left_margin)));
    }
    for (i, col) in cols.iter().enumerate() {
        if i > 0 {
            chain_spans.push(Span::styled(
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
        chain_spans.push(Span::styled(col.node_text.clone(), style));
    }

    let chain_width = cols
        .last()
        .map(|c| c.start_col + visible_width(&c.node_text))
        .unwrap_or(0);

    // Feedback arcs (drawn under the chain).
    let arcs = collect_feedback_arcs(stages, &cols);
    let capped: Vec<_> = arcs.iter().take(MAX_FEEDBACK_ROWS).collect();
    let arc_pairs: Vec<(String, String)> = capped
        .iter()
        .map(|arc| render_feedback_row(arc.source_col, arc.target_col, g))
        .collect();

    // All emitted lines share the same width so centered alignment is neutral.
    let target_inner = inner_width.max(usable_width);
    let chain_total = left_margin + chain_width;
    let chain_pad = target_inner.saturating_sub(chain_total);
    if chain_pad > 0 {
        chain_spans.push(Span::raw(" ".repeat(chain_pad)));
    } else if right_margin > 0 {
        chain_spans.push(Span::raw(" ".repeat(right_margin)));
    }

    let mut lines: Vec<Line<'a>> = Vec::new();
    lines.push(Line::from(chain_spans));

    let arc_style = Style::default().fg(Color::Red);
    for (arc_line, ann_line) in arc_pairs {
        lines.push(dag_arc_line(&ann_line, left_margin, target_inner, arc_style));
        lines.push(dag_arc_line(&arc_line, left_margin, target_inner, arc_style));
    }
    if arcs.len() > MAX_FEEDBACK_ROWS {
        let overflow = arcs.len() - MAX_FEEDBACK_ROWS;
        let text = format!("+{overflow} more feedback edges");
        let padding = target_inner.saturating_sub(visible_width(&text) + left_margin);
        let mut spans: Vec<Span<'a>> = Vec::new();
        if left_margin > 0 {
            spans.push(Span::raw(" ".repeat(left_margin)));
        }
        spans.push(Span::raw(text));
        if padding > 0 {
            spans.push(Span::raw(" ".repeat(padding)));
        }
        lines.push(Line::from(spans));
    }

    lines
}

/// Frame a feedback-arc string with the DAG's left margin and right padding
/// so the line's total visible width equals the pane inner width.
fn dag_arc_line<'a>(
    text: &str,
    left_margin: usize,
    target_inner: usize,
    style: Style,
) -> Line<'a> {
    let body_w = visible_width(text);
    let mut spans: Vec<Span<'a>> = Vec::new();
    if left_margin > 0 {
        spans.push(Span::raw(" ".repeat(left_margin)));
    }
    spans.push(Span::styled(text.to_string(), style));
    let used = left_margin + body_w;
    let pad = target_inner.saturating_sub(used);
    if pad > 0 {
        spans.push(Span::raw(" ".repeat(pad)));
    }
    Line::from(spans)
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

fn render_narrow<'a>(
    stages: &'a [StageDefinition],
    counts: &'a [usize],
    active: Option<&str>,
    g: &'a GlyphSet,
    inner_width: usize,
    definition: &WorkflowDefinition,
) -> Vec<Line<'a>> {
    // Wrap the compact `name(count) → name(count) → …` form across however many
    // rows are needed to fit within `inner_width`. The previous fixed two-row
    // split silently dropped stages from view when the workflow had enough
    // stages that even half the list overflowed one row (AC-1, AC-2).
    //
    // Per-stage styling mirrors the Wide tier: each stage name is colored via
    // `stage_color_for` + BOLD, and the active stage layers REVERSED on top.
    // Inter-stage arrows (` → `) live in DarkGray to keep them readable as
    // connective tissue without competing with the stage colors. At row
    // breaks we emit a trailing arrow on the wrapping row and a leading
    // arrow on the next row so the directed sequence still reads across
    // wraps (the captain feedback in cycle 1).
    //
    // Cycle 2 captain feedback: each finished row also has its slack
    // distributed across the inter-stage arrows so the row content spans
    // the full inner_width (no large empty gap on the right with the
    // centered Paragraph alignment).
    // Cycle 3: render into ~90% of inner_width with the remaining 10% used
    // as left+right margin. All wrap/slack math operates on `usable_width`;
    // the line is then framed by left/right blank spans so the total visible
    // width still equals `inner_width` (the Paragraph uses Alignment::Center
    // — making the line exactly inner_width keeps centering a no-op).
    let usable_width = usable_inner_width(inner_width);
    let (left_margin, right_margin) = horizontal_margins(inner_width, usable_width);

    let arrow_sep = format!(" {} ", g.narrow_arrow);
    let arrow_w = visible_width(&arrow_sep);
    let wrap_trailing = format!(" {}", g.narrow_arrow);
    let wrap_trailing_w = visible_width(&wrap_trailing);
    let wrap_leading = format!("{} ", g.narrow_arrow);
    let wrap_leading_w = visible_width(&wrap_leading);

    struct Segment<'s> {
        stage_name: &'s str,
        count_suffix: String,
        node_text: String,
        is_active: bool,
        total_width: usize,
    }

    let segments: Vec<Segment<'_>> = stages
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            let count = counts.get(i).copied().unwrap_or(0);
            let node_text = build_node_text(stage, g);
            let count_suffix = format!("({count})");
            let total_width = visible_width(&node_text) + visible_width(&count_suffix);
            let is_active = active.map(|s| s == stage.name).unwrap_or(false);
            Segment {
                stage_name: stage.name.as_str(),
                count_suffix,
                node_text,
                is_active,
                total_width,
            }
        })
        .collect();

    // Pre-pack the segments into rows (record indices only) so each row's
    // total width and the number of inter-stage gaps can be known before we
    // emit spans. We need that knowledge to widen each row's gaps to span
    // inner_width — the existing renderer emitted spans as it walked, which
    // left a large trailing void with `Alignment::Center`.
    struct RowPlan {
        first_seg: usize,
        last_seg: usize,
        has_wrap_leading: bool,
        has_wrap_trailing: bool,
        content_width: usize,
    }

    let mut rows: Vec<RowPlan> = Vec::new();
    let mut cur_first: usize = 0;
    let mut cur_last: Option<usize> = None;
    let mut cur_width: usize = 0;
    let mut cur_leading = false;
    for (i, seg) in segments.iter().enumerate() {
        let need_arrow = cur_last.is_some();
        let candidate = if need_arrow {
            cur_width + arrow_w + seg.total_width
        } else {
            cur_width + seg.total_width
        };
        if need_arrow && candidate > usable_width {
            // Wrap. Close out the current row with a trailing wrap arrow if
            // there's room.
            let has_trailing = cur_width + wrap_trailing_w <= usable_width;
            let final_width = cur_width + if has_trailing { wrap_trailing_w } else { 0 };
            rows.push(RowPlan {
                first_seg: cur_first,
                last_seg: cur_last.unwrap(),
                has_wrap_leading: cur_leading,
                has_wrap_trailing: has_trailing,
                content_width: final_width,
            });
            // Start the next row, optionally with a leading wrap arrow.
            cur_first = i;
            cur_last = None;
            cur_width = 0;
            cur_leading = wrap_leading_w + seg.total_width <= usable_width;
            if cur_leading {
                cur_width += wrap_leading_w;
            }
        }
        if cur_last.is_some() {
            cur_width += arrow_w;
        }
        cur_width += seg.total_width;
        cur_last = Some(i);
    }
    if let Some(last) = cur_last {
        rows.push(RowPlan {
            first_seg: cur_first,
            last_seg: last,
            has_wrap_leading: cur_leading,
            has_wrap_trailing: false,
            content_width: cur_width,
        });
    }

    let connector_style = Style::default().fg(Color::DarkGray);
    let mut lines: Vec<Line<'a>> = Vec::with_capacity(rows.len() * (1 + INTER_ROW_PADDING_LINES));

    let last_row_idx = rows.len().saturating_sub(1);
    for (row_idx, plan) in rows.iter().enumerate() {
        // Distribute slack across the inter-stage arrows in this row so the
        // row spans usable_width. Each gap absorbs `slack / gap_count` extra
        // spaces; the first `slack % gap_count` gaps absorb one more.
        let segs_in_row = plan.last_seg - plan.first_seg + 1;
        let gap_count = segs_in_row.saturating_sub(1);
        let slack = usable_width.saturating_sub(plan.content_width);
        let base_extra = slack.checked_div(gap_count).unwrap_or(0);
        let extra_remainder = slack.checked_rem(gap_count).unwrap_or(0);

        let mut spans: Vec<Span<'a>> = Vec::new();
        // Left margin: blank padding so the graph isn't flush against the
        // pane edge (cycle 3 captain feedback).
        if left_margin > 0 {
            spans.push(Span::raw(" ".repeat(left_margin)));
        }
        if plan.has_wrap_leading {
            spans.push(Span::styled(wrap_leading.clone(), connector_style));
        }
        for i in 0..segs_in_row {
            if i > 0 {
                let extra = base_extra + usize::from(i <= extra_remainder);
                let sep = if extra == 0 {
                    arrow_sep.clone()
                } else {
                    format!(" {}{} ", " ".repeat(extra), g.narrow_arrow)
                };
                spans.push(Span::styled(sep, connector_style));
            }
            let seg = &segments[plan.first_seg + i];
            let mut stage_style = Style::default()
                .fg(definition.stage_color_for(seg.stage_name))
                .add_modifier(Modifier::BOLD);
            if seg.is_active {
                stage_style = stage_style.add_modifier(Modifier::REVERSED);
            }
            spans.push(Span::styled(seg.node_text.clone(), stage_style));
            spans.push(Span::styled(seg.count_suffix.clone(), Style::default()));
        }
        if plan.has_wrap_trailing {
            spans.push(Span::styled(wrap_trailing.clone(), connector_style));
        }
        // If there are no gaps to absorb slack (single-segment row) or the
        // gap count is zero, fall back to right-padding the row so the
        // row content occupies usable_width.
        if gap_count == 0 {
            let pad = usable_width.saturating_sub(plan.content_width);
            if pad > 0 {
                spans.push(Span::raw(" ".repeat(pad)));
            }
        }
        // Right margin: keeps total span width == inner_width so the
        // centered Paragraph stays neutral and the graph sits inside a
        // visible right-side margin.
        if right_margin > 0 {
            spans.push(Span::raw(" ".repeat(right_margin)));
        }
        lines.push(Line::from(spans));

        // Inter-row vertical padding: >=3 blank spacer lines between any two
        // stage-bearing rows (cycle 3 captain feedback). Skip after the last
        // row so the feedback-annotation tail line still sits adjacent to
        // the final stage row.
        if row_idx < last_row_idx {
            for _ in 0..INTER_ROW_PADDING_LINES {
                lines.push(Line::default());
            }
        }
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

fn very_narrow_cell_text(stage: &StageDefinition, count: usize, g: &GlyphSet) -> String {
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
    format!("{marker_prefix}{} ({count})", stage.name)
}

fn render_very_narrow<'a>(
    stages: &'a [StageDefinition],
    counts: &'a [usize],
    active: Option<&str>,
    g: &'a GlyphSet,
    inner_width: usize,
    inner_height: usize,
    definition: &WorkflowDefinition,
) -> Vec<Line<'a>> {
    // Build the per-stage cell text once, in stage order.
    let cells: Vec<String> = stages
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            let count = counts.get(i).copied().unwrap_or(0);
            very_narrow_cell_text(stage, count, g)
        })
        .collect();

    if cells.is_empty() {
        return Vec::new();
    }

    // Cycle 3: render into ~90% of inner_width with the remaining 10% used
    // as left+right margin. Slack distribution + width-driven column count
    // both operate on `usable_width`; each emitted line is then framed by
    // left/right blank spans so the total visible width equals inner_width.
    let usable_width = usable_inner_width(inner_width);
    let (left_margin, right_margin) = horizontal_margins(inner_width, usable_width);

    // Cycle 2 captain feedback: feedback-annotation lines (e.g.
    // `↩ rollback on reject: review → implement`) must be rendered in the
    // VeryNarrow tier too, AND the grid height budget must subtract those
    // lines BEFORE choosing how many stage rows fit — otherwise the grid
    // silently consumes the row the annotation needs.
    let fb_parts = feedback_annotations(stages, g);
    // The feedback annotations are emitted as a single line joined by ", "
    // (matching the Narrow tier rendering). Reserve one line for it.
    let feedback_rows = usize::from(!fb_parts.is_empty());
    let grid_height_budget = inner_height.saturating_sub(feedback_rows);
    let grid_height_budget = grid_height_budget.max(1);

    // Inter-cell glue: a narrow arrow (` → `) between adjacent cells within a
    // row, and a trailing/leading arrow at row breaks to keep the directed
    // sequence legible across wraps (captain feedback cycle 1).
    let arrow_sep = format!(" {} ", g.narrow_arrow);
    let col_gap = visible_width(&arrow_sep);

    // Decide a multi-column layout that fits as many stages as possible inside
    // the pane. We reserve one line for a potential overflow indicator. We
    // search column counts from many to few so we maximise stages-per-row when
    // the pane is short.
    let widest_cell = cells.iter().map(|s| visible_width(s)).max().unwrap_or(1);
    let max_cols_by_width = if usable_width == 0 {
        1
    } else {
        ((usable_width + col_gap) / (widest_cell + col_gap)).max(1)
    };
    let max_cols = max_cols_by_width.min(stages.len());

    // Cycle 3 captain feedback: subtract inter-row blank padding from the
    // grid's row budget BEFORE choosing how many rows fit. Each pair of
    // consecutive stage rows costs `INTER_ROW_PADDING_LINES` extra lines, so
    // `R` stage rows actually occupy `R + (R-1) * pad` lines.  Solve for
    // the max R: `R_max = floor((budget + pad) / (1 + pad))`.
    let pad_lines = INTER_ROW_PADDING_LINES;
    let max_rows_with_padding = grid_height_budget
        .saturating_add(pad_lines)
        .checked_div(1 + pad_lines)
        .unwrap_or(1)
        .max(1);

    // For each candidate column count, compute the row count and pick the
    // largest column count that still fits within `max_rows_with_padding`.
    let mut chosen_cols = 1usize;
    let mut chosen_rows = stages.len();
    let mut found = false;
    for cols in (1..=max_cols).rev() {
        let rows = stages.len().div_ceil(cols);
        if rows <= max_rows_with_padding {
            chosen_cols = cols;
            chosen_rows = rows;
            found = true;
            break;
        }
    }
    if !found {
        // Nothing fits — fall through to overflow logic with the densest
        // (max_cols) layout so we hide as little as possible.
        chosen_cols = max_cols;
        chosen_rows = stages.len().div_ceil(chosen_cols);
    }

    // If even the densest tried layout doesn't fit, fall back to the maximum
    // columns and let overflow logic trim. We may also need to leave a line
    // for the overflow indicator.
    if chosen_rows > max_rows_with_padding {
        chosen_cols = max_cols;
        chosen_rows = stages.len().div_ceil(chosen_cols);
    }

    let visible_rows = chosen_rows.min(max_rows_with_padding);
    let visible_cells = visible_rows * chosen_cols;
    let need_overflow = visible_cells < stages.len();

    // If we need an overflow line, reserve one row for it (provided we have
    // more than one row to work with). Recompute visible cells accordingly.
    let (visible_rows, visible_cells, need_overflow) = if need_overflow && visible_rows > 1 {
        let r = visible_rows - 1;
        let v = r * chosen_cols;
        (r, v, v < stages.len())
    } else {
        (visible_rows, visible_cells, need_overflow)
    };

    // Cycle 2 captain feedback: distribute the horizontal slack between the
    // chosen columns so the grid spans the entire pane width (no large
    // left-side empty gap under the centered Paragraph alignment).
    //
    // Layout math is per-row because rows differ on whether they carry a
    // wrap_leading (continuation rows) and/or a wrap_trailing (rows that
    // are followed by more rows in the grid). For each row we compute:
    //   baseline = wrap_leading_w + N*widest_cell + (N-1)*col_gap + wrap_trailing_w
    // and distribute (inner_width - baseline) across the inter-cell gaps.
    // If there are zero gaps (single-cell row), the slack right-pads.
    let col_width = widest_cell;
    let connector_style = Style::default().fg(Color::DarkGray);
    let wrap_trailing = format!(" {}", g.narrow_arrow);
    let wrap_trailing_w = visible_width(&wrap_trailing);
    let wrap_leading = format!("{} ", g.narrow_arrow);
    let wrap_leading_w = visible_width(&wrap_leading);
    let mut lines: Vec<Line<'a>> =
        Vec::with_capacity(visible_rows * (1 + INTER_ROW_PADDING_LINES) + 2);
    // Track the last rendered stage row index so we know where to insert
    // inter-row padding (cycle 3 captain feedback).
    let mut last_stage_row_pushed: Option<usize> = None;
    for r in 0..visible_rows {
        // How many cells does this row hold?
        let row_first_idx = r * chosen_cols;
        let row_last_excl = ((r + 1) * chosen_cols).min(visible_cells).min(cells.len());
        let n_in_row = row_last_excl.saturating_sub(row_first_idx);
        if n_in_row == 0 {
            continue;
        }
        // Inter-row blank padding before every stage row except the first
        // rendered one. Pure spacers — no glyph.
        if last_stage_row_pushed.is_some() {
            for _ in 0..INTER_ROW_PADDING_LINES {
                lines.push(Line::default());
            }
        }

        let has_lead = r > 0;
        // Cycle 4 captain feedback: inter-stage / wrap_trailing arrows only
        // render BETWEEN two real stage cells, never after the very last
        // emitted stage cell. A trailing wrap arrow is therefore added only
        // when at least one more stage cell will actually be rendered in a
        // subsequent visible row — `row_last_excl < visible_cells`. The
        // previous condition also pointed an arrow at the `+N hidden:`
        // overflow indicator, which the captain rejected.
        let has_trail = row_last_excl < visible_cells;
        let lead_w = if has_lead { wrap_leading_w } else { 0 };
        let trail_w = if has_trail { wrap_trailing_w } else { 0 };
        let baseline =
            lead_w + n_in_row * col_width + n_in_row.saturating_sub(1) * col_gap + trail_w;
        let slack = usable_width.saturating_sub(baseline);
        let gap_count = n_in_row.saturating_sub(1);
        let base_extra_gap = slack.checked_div(gap_count).unwrap_or(0);
        let extra_gap_remainder = slack.checked_rem(gap_count).unwrap_or(0);

        let mut spans: Vec<Span<'a>> = Vec::new();
        // Left margin: cycle 3 captain feedback.
        if left_margin > 0 {
            spans.push(Span::raw(" ".repeat(left_margin)));
        }
        // Leading wrap indicator on continuation rows so the directed
        // sequence reads across row breaks.
        if has_lead {
            spans.push(Span::styled(wrap_leading.clone(), connector_style));
        }
        for c in 0..n_in_row {
            let idx = row_first_idx + c;
            if c > 0 {
                let extra = base_extra_gap + usize::from(c <= extra_gap_remainder);
                let sep = if extra == 0 {
                    arrow_sep.clone()
                } else {
                    format!(" {}{} ", " ".repeat(extra), g.narrow_arrow)
                };
                spans.push(Span::styled(sep, connector_style));
            }
            let cell = &cells[idx];
            let pad = col_width.saturating_sub(visible_width(cell));
            let is_active = active.map(|s| s == stages[idx].name).unwrap_or(false);
            let mut style = Style::default()
                .fg(definition.stage_color_for(&stages[idx].name))
                .add_modifier(Modifier::BOLD);
            if is_active {
                style = style.add_modifier(Modifier::REVERSED);
            }
            spans.push(Span::styled(cell.clone(), style));
            if pad > 0 {
                spans.push(Span::raw(" ".repeat(pad)));
            }
        }
        if has_trail {
            spans.push(Span::styled(wrap_trailing.clone(), connector_style));
        }
        // Single-cell rows (n_in_row == 1): right-pad to span usable_width.
        if gap_count == 0 {
            let total = lead_w + col_width + trail_w;
            let pad_right = usable_width.saturating_sub(total);
            if pad_right > 0 {
                spans.push(Span::raw(" ".repeat(pad_right)));
            }
        }
        // Right margin: cycle 3 captain feedback.
        if right_margin > 0 {
            spans.push(Span::raw(" ".repeat(right_margin)));
        }
        lines.push(Line::from(spans));
        last_stage_row_pushed = Some(r);
    }

    if need_overflow {
        let hidden: Vec<&str> = stages[visible_cells..]
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        // Compose the indicator and trim names if it would itself overflow.
        let prefix = format!("+{} hidden: ", hidden.len());
        let mut joined = String::new();
        let mut shown = 0usize;
        for (i, name) in hidden.iter().enumerate() {
            let candidate = if i == 0 {
                name.to_string()
            } else {
                format!(", {name}")
            };
            if visible_width(&prefix) + visible_width(&joined) + visible_width(&candidate)
                > inner_width
            {
                break;
            }
            joined.push_str(&candidate);
            shown += 1;
        }
        let text = if shown < hidden.len() {
            format!("{prefix}{joined}, …")
        } else {
            format!("{prefix}{joined}")
        };
        lines.push(Line::from(Span::styled(
            text,
            Style::default().add_modifier(Modifier::DIM),
        )));
    }

    // Feedback annotations live on a reserved trailing line so the
    // ↩ rollback notice is always visible when the workflow declares
    // `feedback-to:` paths (matches Narrow tier behaviour).
    if !fb_parts.is_empty() {
        lines.push(Line::from(fb_parts.join(", ")));
    }

    lines
}

#[cfg(test)]
mod tests;
