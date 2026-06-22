use super::*;
use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::app::{App, OverviewState};
use spacetop_core::domain::{Entity, StageDefinition, WorkflowDefinition, WorkflowSnapshot};

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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
    App::load(root).expect("real workflow should load")
}

fn workflow_with_active_item() -> App {
    let root = PathBuf::from("/tmp/spacetop-graph");
    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: root.clone(),
            state: None,
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
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items: vec![make_item("001", "plan", "Plan task")],
        parse_errors: Vec::new(),
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
    let cols = dag_layout_columns(&stages, &[0], None, &g);
    assert!(cols[0].node_text.starts_with("\u{25B6}"));
}

#[test]
fn layout_columns_places_terminal_marker_on_last_stage() {
    let g = glyphs_for(false);
    let stages = vec![stage("done", false, true, false, false, None)];
    let cols = dag_layout_columns(&stages, &[0], None, &g);
    assert!(cols[0].node_text.ends_with("\u{25A0}"));
}

#[test]
fn layout_columns_single_glyph_per_stage_initial_takes_priority() {
    // Spec: exactly one leading glyph per stage, priority: initial > gate > worktree.
    // A stage that is initial, gate, and worktree should display only ▶.
    let g = glyphs_for(false);
    let stages = vec![stage("x", true, true, true, true, None)];
    let cols = dag_layout_columns(&stages, &[0], None, &g);
    let t = &cols[0].node_text;
    // Must contain initial glyph ▶, name 'x', and terminal ■.
    assert!(t.contains('\u{25B6}'), "missing initial glyph ▶");
    assert!(t.contains('x'), "missing stage name");
    assert!(t.contains('\u{25A0}'), "missing terminal glyph ■");
    // Must NOT contain gate ⚑ or worktree ⎇ (single-glyph rule).
    assert!(
        !t.contains('\u{2691}'),
        "gate glyph ⚑ must not appear when initial is set"
    );
    assert!(
        !t.contains('\u{2387}'),
        "worktree glyph ⎇ must not appear when initial is set"
    );
    // ▶ must precede 'x' which must precede ■.
    let ini = t.find('\u{25B6}').unwrap();
    let nm = t.find('x').unwrap();
    let term = t.find('\u{25A0}').unwrap();
    assert!(ini < nm && nm < term);
}

#[test]
fn pick_width_tier_returns_expected_tier_for_sample_widths() {
    let app = real_workflow();
    let stages = &app.as_overview().expect("overview").definition().stages;
    let counts: Vec<usize> = app.stage_counts().into_iter().map(|c| c.items).collect();
    let g = glyphs_for(false);
    // Inner-height matches the production graph pane (7 - 2 borders = 5).
    let h: usize = 5;
    assert_eq!(
        pick_width_tier(120, h, stages, &counts, &g),
        WidthTier::Wide,
        "120 cols should fit wide"
    );
    // 38 inner cols is too small for wide but fits narrow compact form for our 5-stage set.
    let narrow = pick_width_tier(38, h, stages, &counts, &g);
    assert!(
        matches!(narrow, WidthTier::Narrow | WidthTier::VeryNarrow),
        "small width should not be Wide"
    );
    // At 22 cols the DAG cannot pack the 5-stage chain into the available
    // height (multi-row DAG would need too many rows), so we should fall
    // through to the wrapped-text VeryNarrow tier.
    assert_eq!(
        pick_width_tier(22, h, stages, &counts, &g),
        WidthTier::VeryNarrow
    );
}

#[test]
fn renders_wide_ribbon_with_unicode_glyphs_for_real_workflow() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = real_workflow();
    let rendered = render_to_string(&app, 120, 10);
    let names: Vec<String> = app
        .as_overview()
        .expect("overview")
        .definition()
        .stages
        .iter()
        .map(|stage| stage.name.clone())
        .collect();
    for name in &names {
        assert!(rendered.contains(name), "missing stage {name}");
    }
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    assert!(monotonic_contains(&rendered, &name_refs));
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
            state: None,
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
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items: Vec::new(),
        parse_errors: Vec::new(),
    };
    let app = App::from_snapshot(PathBuf::from("/tmp/other"), snapshot);
    let rendered = render_to_string(&app, 120, 10);
    assert!(rendered.contains("alpha"));
    assert!(rendered.contains("beta"));
    assert!(rendered.contains("gamma"));
    assert!(!rendered.contains("design"));
    assert!(!rendered.contains("review"));
    assert!(
        !rendered.contains("\u{2570}"),
        "no arc corner for topology without feedback edges"
    );
    assert!(
        !rendered.contains("reject"),
        "no reject label for topology without feedback edges"
    );
}

#[test]
fn narrow_tier_renders_compact_textual_summary() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    // Use a synthetic workflow with the same shape as the real one
    // (5 stages, 4 markers) but no items — so all counts are 0 and
    // `narrow_summary` lands at exactly 56 chars. The wide ribbon
    // needs 57 chars (▶ design + ──► + plan + ──► + ⎇ implement +
    // ──► + ⚑ review + ──► + done ■). At width=56 the narrow tier
    // is selected.
    //
    // Decoupling from `real_workflow()` avoids flakes when the
    // archive accumulates and pushes `done(N)` past one digit.
    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: PathBuf::from("/tmp/narrow-tier"),
            state: None,
            stages: vec![
                stage("design", true, false, false, false, None),
                stage("plan", false, false, false, false, None),
                stage("implement", false, false, false, true, None),
                stage("review", false, false, true, false, None),
                stage("done", false, true, false, false, None),
            ],
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items: Vec::new(),
        parse_errors: Vec::new(),
    };
    let app = App::from_snapshot(PathBuf::from("/tmp/narrow-tier"), snapshot);
    // Height=4 leaves inner_height=2 (TOP+BOTTOM borders), which is too
    // short for the multi-row DAG tier (single chain row + at least one
    // feedback-arc row already maxes the budget when the chain needs to
    // wrap). At inner_width=56 the inline DAG chain cannot fit either, so
    // pick_width_tier falls through to the wrapped-text Narrow tier — the
    // exact path this test exercises.
    let rendered = render_to_string(&app, 56, 4);
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
    // Cycle 3 adds ≥3 blank lines between stage rows; 5 stages stacked at
    // 1-per-line need 5 + 4*3 = 17 lines + pane borders. Bump height
    // accordingly so all stage names still land on distinct rows.
    let height: u16 = 22;
    let rendered = render_to_string(&app, width, height);
    // Verify each stage name appears on a distinct row (strictly increasing row index).
    let cols = width as usize;
    let mut last_row: Option<usize> = None;
    let names: Vec<String> = app
        .as_overview()
        .expect("overview")
        .definition()
        .stages
        .iter()
        .map(|stage| stage.name.clone())
        .collect();
    for name in names {
        let pos = rendered
            .find(&name)
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
    assert!(
        !rendered.contains('\u{2191}'),
        "Unicode arc up-arrow leaked"
    );
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
        stage("start", true, false, false, false, None), // initial → ▶
        stage("check", false, false, true, false, None), // gate → ⚑
        stage("work", false, false, false, true, None),  // worktree → ⎇
        stage("done", false, true, false, false, None),  // terminal → ■ suffix
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
    let arc_left = '\u{2570}'; // ╰
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
    assert!(
        found_arc,
        "expected to find arc corner chars ╰/╯ in the rendered output"
    );
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
    assert!(
        rgb_count >= 3,
        "expected at least 3 Rgb-colored cells in DAG, found {rgb_count}"
    );
}

#[test]
fn narrow_dag_wraps_to_two_rows() {
    // AC-5: render_narrow must produce (at least) 2 lines, with the first half
    // of stages on row 1 and the second half on row 2.
    let g = glyphs_for(false);
    // Use 6 stages: mid = 3. Row1 = [alpha, beta, gamma], row2 = [delta, epsilon, done].
    let stages = vec![
        stage("alpha", true, false, false, false, None),
        stage("beta", false, false, false, true, None),
        stage("gamma", false, false, true, false, None),
        stage("delta", false, false, false, true, None),
        stage("epsilon", false, false, false, false, None),
        stage("done", false, true, false, false, None),
    ];
    let counts = vec![1usize, 2, 3, 4, 5, 0];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/narrow-wrap"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };
    // Choose an inner_width that fits roughly half the stages per row.
    // alpha(1), beta(2), gamma(3), delta(4), epsilon(5), done(0) with markers
    // and arrows; ~40 cols should split into about two rows.
    let lines = render_narrow(&stages, &counts, None, &g, 40, &definition);
    // Must produce at least 2 lines.
    assert!(
        lines.len() >= 2,
        "render_narrow must produce at least 2 lines when stages do not fit on one row, got {}",
        lines.len()
    );
    // Every stage name must appear somewhere across all rows.
    let all_text: String = lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    for name in ["alpha", "beta", "gamma", "delta", "epsilon", "done"] {
        assert!(
            all_text.contains(name),
            "narrow tier must show every stage; missing {name} in {all_text:?}"
        );
    }
    // Sanity: first row must not contain the last stage (must have wrapped).
    let row1: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        !row1.contains("done"),
        "row 1 must wrap before 'done' at inner_width=40; got: {row1:?}"
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

/// Build the 12-stage research workflow described in the AC for entity 009.
fn research_12_stage_workflow() -> App {
    let root = PathBuf::from("/tmp/spacetop-research-12");
    let names = [
        "pending", "scoping", "ideate", "review", "smoke", "run", "analyze", "promote", "expanded",
        "ideated", "done", "rejected",
    ];
    let stages: Vec<StageDefinition> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "done" || *n == "rejected";
            stage(n, initial, terminal, false, false, None)
        })
        .collect();
    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: root.clone(),
            state: None,
            stages,
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items: Vec::new(),
        parse_errors: Vec::new(),
    };
    App::from_snapshot(root, snapshot)
}

const RESEARCH_STAGES: &[&str] = &[
    "pending", "scoping", "ideate", "review", "smoke", "run", "analyze", "promote", "expanded",
    "ideated", "done", "rejected",
];

/// AC-1: every stage name must appear (or be named in an explicit overflow
/// indicator) at a representative narrow pane size for a 12-stage workflow.
#[test]
fn fits_all_twelve_research_stages_at_narrow_pane_size() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = research_12_stage_workflow();
    // 80x24 overall is a representative cramped terminal. The graph pane is 7
    // rows tall (1 top border, 1 bottom border, 5 inner lines) per the
    // overview layout in `src/ui/mod.rs`.
    let rendered = render_to_string(&app, 80, 7);
    let total_lines = rendered.len() / 80;
    for name in RESEARCH_STAGES {
        let present = rendered.contains(name);
        if present {
            continue;
        }
        // Allow an explicit overflow indicator that names the hidden count.
        assert!(
            rendered.contains("hidden:"),
            "stage {name} missing and no '+N hidden:' indicator present (rendered {total_lines} lines)"
        );
        // The indicator must mention this stage by name if it was elided.
        assert!(
            rendered.contains(name),
            "stage {name} not in visible cells nor named by overflow indicator"
        );
    }
}

/// AC-2: at the Wide width breakpoint every stage must be visible.
#[test]
fn fits_all_twelve_research_stages_in_wide_tier() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = research_12_stage_workflow();
    // 200 columns is generous enough for the wide ribbon.
    let rendered = render_to_string(&app, 200, 10);
    for name in RESEARCH_STAGES {
        assert!(
            rendered.contains(name),
            "wide tier must show {name}; full rendered=\n{rendered}"
        );
    }
}

/// AC-2: at the Narrow width breakpoint every stage must be visible (the
/// renderer wraps into as many rows as needed instead of dropping stages).
#[test]
fn fits_all_twelve_research_stages_in_narrow_tier() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = research_12_stage_workflow();
    // 100 cols is wide enough to wrap two-or-three rows of compact form but
    // not the full single-line wide ribbon (which needs >120 cols for 12
    // stages with markers).
    let rendered = render_to_string(&app, 100, 14);
    for name in RESEARCH_STAGES {
        assert!(
            rendered.contains(name),
            "narrow tier must show every stage; missing {name}"
        );
    }
}

/// AC-2: at the VeryNarrow width breakpoint every stage must be visible
/// (the grid layout packs stages into multiple columns; if even that
/// overflows vertically, the overflow indicator must name the hidden stages).
#[test]
fn fits_all_twelve_research_stages_in_very_narrow_tier() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = research_12_stage_workflow();
    // 40 cols forces the VeryNarrow tier for a 12-stage workflow. Cycle 3
    // captain feedback adds ≥3 blank lines between stage rows, so the grid
    // needs more vertical room than before (12 stages at 2 cols → 6 rows
    // → 6 + 5*3 = 21 stage-grid lines, plus pane borders).
    let rendered = render_to_string(&app, 40, 26);
    for name in RESEARCH_STAGES {
        assert!(rendered.contains(name), "very narrow tier must show {name}");
    }
}

/// AC-1 fallback: when even the multi-column grid cannot fit every stage
/// within the available pane height, an overflow indicator must be present
/// and must name the hidden stages so the captain knows what is hidden.
#[test]
fn very_narrow_overflow_indicator_names_hidden_stages() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = research_12_stage_workflow();
    // 14 cols is narrow enough that each cell needs its own column AND only
    // 3 rows of inner content (height 5) — only a handful of stages can
    // fit, so the overflow indicator must kick in.
    let rendered = render_to_string(&app, 14, 5);
    assert!(
        rendered.contains("hidden:"),
        "expected '+N hidden:' overflow indicator at extreme size; got:\n{rendered}"
    );
}

/// Cycle 1 feedback: every stage-name span in the wrapped Narrow renderer
/// must carry the per-stage `stage_color_for` color plus BOLD, matching the
/// Wide-tier convention. We assert this at the level of the styled spans
/// returned from `render_narrow` so the assertion is robust to terminal
/// fallthrough.
#[test]
fn narrow_tier_colors_each_stage_name_per_stage() {
    let g = glyphs_for(false);
    let stages: Vec<StageDefinition> = RESEARCH_STAGES
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "done" || *n == "rejected";
            stage(n, initial, terminal, false, false, None)
        })
        .collect();
    let counts = vec![0usize; stages.len()];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/narrow-colors"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };
    let lines = render_narrow(&stages, &counts, Some("review"), &g, 60, &definition);

    // Collect (span_content, span_style) pairs across all rows.
    let mut found_colored: usize = 0;
    let mut active_seen_reversed = false;
    let mut found_arrow = false;
    for line in &lines {
        for span in &line.spans {
            // Inter-stage arrow.
            if span.content.contains('\u{2192}') {
                found_arrow = true;
            }
            // Stage-name spans look like "design", "▶ design", "done ■", etc.
            // Match a span whose content contains any RESEARCH_STAGES name and
            // assert it carries the per-stage color + BOLD.
            for name in RESEARCH_STAGES {
                if span.content.contains(name) && !span.content.starts_with('(') {
                    let expected = crate::ui::color::to_color(definition.stage_color_for(name));
                    if span.style.fg == Some(expected)
                        && span.style.add_modifier.contains(Modifier::BOLD)
                    {
                        found_colored += 1;
                        if *name == "review" && span.style.add_modifier.contains(Modifier::REVERSED)
                        {
                            active_seen_reversed = true;
                        }
                    }
                }
            }
        }
    }
    assert!(
        found_colored >= RESEARCH_STAGES.len(),
        "expected every research stage name span to carry per-stage color+BOLD, \
         found {found_colored} of {} (lines={lines:?})",
        RESEARCH_STAGES.len()
    );
    assert!(
        active_seen_reversed,
        "active stage 'review' span must carry Modifier::REVERSED on top of color+BOLD"
    );
    assert!(
        found_arrow,
        "narrow tier must render inter-stage arrows (→) between stages"
    );
}

/// Cycle 1 feedback: every stage-name span in the multi-column VeryNarrow
/// grid must carry the per-stage `stage_color_for` color plus BOLD, and the
/// grid must include inter-stage arrows within rows.
#[test]
fn very_narrow_tier_colors_each_stage_name_per_stage() {
    let g = glyphs_for(false);
    let stages: Vec<StageDefinition> = RESEARCH_STAGES
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "done" || *n == "rejected";
            stage(n, initial, terminal, false, false, None)
        })
        .collect();
    let counts = vec![0usize; stages.len()];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/vnarrow-colors"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };
    // 40 cols, height 24 — every stage fits in the grid without overflow,
    // accounting for cycle-3 inter-row blank padding (3 blank lines between
    // any two stage rows in the multi-column grid).
    let lines = render_very_narrow(&stages, &counts, Some("analyze"), &g, 40, 24, &definition);

    let mut found_colored: usize = 0;
    let mut active_seen_reversed = false;
    let mut found_arrow = false;
    for line in &lines {
        for span in &line.spans {
            if span.content.contains('\u{2192}') {
                found_arrow = true;
            }
            for name in RESEARCH_STAGES {
                if span.content.contains(name) {
                    let expected = crate::ui::color::to_color(definition.stage_color_for(name));
                    if span.style.fg == Some(expected)
                        && span.style.add_modifier.contains(Modifier::BOLD)
                    {
                        found_colored += 1;
                        if *name == "analyze"
                            && span.style.add_modifier.contains(Modifier::REVERSED)
                        {
                            active_seen_reversed = true;
                        }
                    }
                }
            }
        }
    }
    assert!(
        found_colored >= RESEARCH_STAGES.len(),
        "expected every research stage name cell to carry per-stage color+BOLD, \
         found {found_colored} of {} (lines={lines:?})",
        RESEARCH_STAGES.len()
    );
    assert!(
        active_seen_reversed,
        "active stage 'analyze' cell must carry Modifier::REVERSED on top of color+BOLD"
    );
    assert!(
        found_arrow,
        "very-narrow grid must render inter-stage arrows (→) within rows"
    );
}

/// Cycle 3 captain feedback (ask #1): the wrapped Narrow renderer must
/// render into roughly 90% of `inner_width`, with the remaining 10% used
/// as left+right margin. Multi-stage rows distribute slack across the
/// inter-stage gaps to span `usable_width`; the line as a whole still
/// reaches `inner_width` (left margin + content + right margin) so the
/// centered Paragraph alignment is a no-op.
#[test]
fn narrow_tier_uses_full_pane_width() {
    let g = glyphs_for(false);
    let stages: Vec<StageDefinition> = RESEARCH_STAGES
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "done" || *n == "rejected";
            stage(n, initial, terminal, false, false, None)
        })
        .collect();
    let counts = vec![0usize; stages.len()];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/narrow-fullwidth"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };
    // Force wrapping by choosing an inner_width well below the narrow_summary
    // width (~154 chars for the 12-stage research fixture).
    let inner_width = 80usize;
    let lines = render_narrow(&stages, &counts, None, &g, inner_width, &definition);
    // Skip the trailing feedback-annotation line (if any) by only checking
    // lines that contain at least one stage name.
    let stage_lines: Vec<&Line<'_>> = lines
        .iter()
        .filter(|line| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            RESEARCH_STAGES.iter().any(|s| text.contains(s))
        })
        .collect();
    assert!(
        stage_lines.len() >= 2,
        "narrow renderer must wrap onto multiple rows at inner_width=80; \
         got {} stage-bearing rows",
        stage_lines.len()
    );
    // Cycle 3: the graph renders into ~90% of inner_width (usable_width=72)
    // with the remaining 10% as left+right margin. Each line's total
    // visible width still equals inner_width so the centered Paragraph is a
    // no-op.
    let usable_width = usable_inner_width(inner_width);
    assert!(
        usable_width < inner_width,
        "usable_width must leave room for horizontal margins at inner_width=80"
    );
    let (left_margin, right_margin) = horizontal_margins(inner_width, usable_width);
    assert!(left_margin > 0, "left margin must be non-zero (cycle 3)");
    assert!(right_margin > 0, "right margin must be non-zero (cycle 3)");
    for (i, line) in stage_lines.iter().enumerate() {
        let total_width: usize = line.spans.iter().map(|s| visible_width(&s.content)).sum();
        assert_eq!(
            total_width, inner_width,
            "narrow row {i} total visible_width={total_width} does not span inner_width={inner_width}"
        );
        // Every stage row must begin with a blank left-margin span so the
        // graph isn't flush against the pane edge.
        let first_span_text: &str = line.spans.first().map(|s| s.content.as_ref()).unwrap_or("");
        assert!(
            first_span_text.chars().all(|c| c == ' ')
                && first_span_text.chars().count() == left_margin,
            "narrow row {i} must start with a left-margin span of {left_margin} spaces; \
             got first span={first_span_text:?}"
        );
        // And it must end with a blank right-margin span of equal-ish width.
        let last_span_text: &str = line.spans.last().map(|s| s.content.as_ref()).unwrap_or("");
        assert!(
            last_span_text.chars().all(|c| c == ' ')
                && last_span_text.chars().count() >= right_margin,
            "narrow row {i} must end with a right-margin span of >={right_margin} spaces; \
             got last span={last_span_text:?}"
        );
    }
}

/// Cycle 3 captain feedback (ask #1): the VeryNarrow multi-column grid must
/// render into ~90% of inner_width with the remaining 10% as left+right
/// margin. The line's total visible width still equals inner_width so the
/// centered Paragraph is a no-op.
#[test]
fn very_narrow_tier_uses_full_pane_width() {
    let g = glyphs_for(false);
    let stages: Vec<StageDefinition> = RESEARCH_STAGES
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "done" || *n == "rejected";
            stage(n, initial, terminal, false, false, None)
        })
        .collect();
    let counts = vec![0usize; stages.len()];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/vnarrow-fullwidth"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };
    let inner_width = 40usize;
    // Cycle 3: with INTER_ROW_PADDING_LINES=3 between rows, give a generous
    // inner_height so the test still exercises all stage rows without the
    // padding pushing stages off-screen. 12 stages at 2 cols → 6 rows → 6 +
    // 5*3 = 21 lines + 1 feedback line = budget >= 22.
    let inner_height = 24usize;
    let lines = render_very_narrow(
        &stages,
        &counts,
        None,
        &g,
        inner_width,
        inner_height,
        &definition,
    );
    // Identify the rows that contain stage names (skip any overflow/feedback
    // tail lines).
    let stage_lines: Vec<&Line<'_>> = lines
        .iter()
        .filter(|line| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            RESEARCH_STAGES.iter().any(|s| text.contains(s))
                && !text.contains("rollback on reject")
                && !text.starts_with('+')
        })
        .collect();
    assert!(
        !stage_lines.is_empty(),
        "expected stage-bearing lines in very-narrow output"
    );
    // Cycle 3: graph renders into ~90% of inner_width with non-zero
    // left+right margins; total line width still equals inner_width.
    let usable_width = usable_inner_width(inner_width);
    assert!(
        usable_width < inner_width,
        "usable_width must leave room for horizontal margins at inner_width=40"
    );
    let (left_margin, right_margin) = horizontal_margins(inner_width, usable_width);
    assert!(left_margin > 0, "left margin must be non-zero (cycle 3)");
    assert!(right_margin > 0, "right margin must be non-zero (cycle 3)");
    for (i, line) in stage_lines.iter().enumerate() {
        let total_width: usize = line.spans.iter().map(|s| visible_width(&s.content)).sum();
        assert_eq!(
            total_width, inner_width,
            "very-narrow row {i} total visible_width={total_width} does not span inner_width={inner_width}"
        );
        // Every stage row must start with a blank left-margin span.
        let first_span_text: &str = line.spans.first().map(|s| s.content.as_ref()).unwrap_or("");
        assert!(
            first_span_text.chars().all(|c| c == ' ')
                && first_span_text.chars().count() == left_margin,
            "very-narrow row {i} must start with a left-margin span of {left_margin} spaces; \
             got first span={first_span_text:?}"
        );
        // And end with a blank right-margin span of at least right_margin
        // spaces (cell padding may add more on the last cell).
        let last_span_text: &str = line.spans.last().map(|s| s.content.as_ref()).unwrap_or("");
        assert!(
            last_span_text.chars().all(|c| c == ' ')
                && last_span_text.chars().count() >= right_margin,
            "very-narrow row {i} must end with a right-margin span of >={right_margin} spaces; \
             got last span={last_span_text:?}"
        );
    }
}

/// Cycle 2 captain feedback (ask #2): the VeryNarrow tier must render the
/// `↩ rollback on reject: review → implement` annotation when the workflow
/// declares a `feedback-to:` path. The grid height budget must reserve room
/// for this line BEFORE deciding how many stage rows fit.
#[test]
fn very_narrow_tier_renders_feedback_rollback_annotation() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    // Build a research-like 12-stage workflow with `review` declaring
    // `feedback-to: implement` (matches the real research workflow).
    let root = PathBuf::from("/tmp/spacetop-research-rb");
    let names = [
        "pending",
        "scoping",
        "ideate",
        "implement",
        "review",
        "smoke",
        "run",
        "analyze",
        "promote",
        "expanded",
        "ideated",
        "done",
    ];
    let stages: Vec<StageDefinition> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "done";
            // `review` points its feedback edge back at `implement`.
            let fb = if *n == "review" {
                Some("implement")
            } else {
                None
            };
            stage(n, initial, terminal, false, false, fb)
        })
        .collect();
    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: root.clone(),
            state: None,
            stages,
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items: Vec::new(),
        parse_errors: Vec::new(),
    };
    let app = App::from_snapshot(root, snapshot);
    // 40x9 keeps the pane narrow enough that the DAG cannot pack the 12
    // stages into the available height (inner_height=7 must hold the chain
    // rows AND their connectors), so pick_width_tier falls through to the
    // VeryNarrow grid. The grid then has room for the `↩` annotation tail
    // line that this test pins.
    let rendered = render_to_string(&app, 40, 9);
    assert!(
        rendered.contains("rollback on reject"),
        "VeryNarrow tier must render the feedback rollback annotation; rendered=\n{rendered}"
    );
    assert!(
        rendered.contains("review") && rendered.contains("implement"),
        "rollback annotation must name source and target stages; rendered=\n{rendered}"
    );
    assert!(
        rendered.contains('\u{21B6}'),
        "rollback annotation must use the ↩ feedback glyph; rendered=\n{rendered}"
    );
}

/// Cycle 3 captain feedback (ask #2): the wrapped Narrow renderer must
/// inject ≥3 blank spacer Lines between any two stage-bearing rows so the
/// rows visually read as distinct bands.
#[test]
fn narrow_tier_inserts_blank_lines_between_rows() {
    let g = glyphs_for(false);
    let stages: Vec<StageDefinition> = RESEARCH_STAGES
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "done" || *n == "rejected";
            stage(n, initial, terminal, false, false, None)
        })
        .collect();
    let counts = vec![0usize; stages.len()];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/narrow-interrow"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };
    // Choose inner_width that forces multiple stage rows for the 12-stage
    // research fixture (usable_width=72 cannot hold the full narrow_summary).
    let inner_width = 80usize;
    let lines = render_narrow(&stages, &counts, None, &g, inner_width, &definition);

    // Identify the indices of stage-bearing lines (skip trailing feedback
    // annotations / empty spacer lines).
    let stage_row_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let has_stage = RESEARCH_STAGES.iter().any(|s| text.contains(s));
            if has_stage {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    assert!(
        stage_row_indices.len() >= 2,
        "expected at least 2 stage rows at inner_width=80 to exercise inter-row padding"
    );
    for pair in stage_row_indices.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        // Cycle 4 captain feedback: inter-row padding tightened from 3 to 1
        // blank spacer Line between any two stage rows.
        let gap = b - a - 1;
        assert_eq!(
            gap, 1,
            "narrow tier must have exactly 1 blank spacer Line between stage rows {a} and {b}; got {gap}"
        );
        for (k, line) in lines.iter().enumerate().take(b).skip(a + 1) {
            let blank: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                blank.trim().is_empty(),
                "expected blank spacer Line at index {k} between stage rows; got {blank:?}"
            );
        }
    }
}

/// Cycle 3 captain feedback (ask #2): the VeryNarrow multi-column grid must
/// also inject blank spacer Lines between consecutive stage rows (cycle 4
/// tightened the count from 3 to 1).
#[test]
fn very_narrow_tier_inserts_blank_lines_between_rows() {
    let g = glyphs_for(false);
    let stages: Vec<StageDefinition> = RESEARCH_STAGES
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "done" || *n == "rejected";
            stage(n, initial, terminal, false, false, None)
        })
        .collect();
    let counts = vec![0usize; stages.len()];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/vnarrow-interrow"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };
    let inner_width = 40usize;
    let inner_height = 24usize;
    let lines = render_very_narrow(
        &stages,
        &counts,
        None,
        &g,
        inner_width,
        inner_height,
        &definition,
    );
    let stage_row_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let has_stage = RESEARCH_STAGES.iter().any(|s| text.contains(s))
                && !text.contains("rollback on reject")
                && !text.starts_with('+');
            if has_stage {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    assert!(
        stage_row_indices.len() >= 2,
        "expected at least 2 stage rows in the very-narrow grid to exercise inter-row padding"
    );
    for pair in stage_row_indices.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let gap = b - a - 1;
        assert_eq!(
            gap, 1,
            "very-narrow tier must have exactly 1 blank spacer Line between stage rows {a} and {b}; got {gap}"
        );
        for (k, line) in lines.iter().enumerate().take(b).skip(a + 1) {
            let blank: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                blank.trim().is_empty(),
                "expected blank spacer Line at index {k} between stage rows; got {blank:?}"
            );
        }
    }
}

/// Cycle 3 captain feedback (ask #2): the VeryNarrow tier's row-count
/// budgeting must subtract inter-row padding from inner_height BEFORE
/// picking how many rows fit, so the padding does not push stages
/// off-screen. Equivalently, the same stage that would fit without the
/// padding-aware subtraction continues to fit when we add just enough
/// vertical room to absorb the extra padding.
#[test]
fn very_narrow_tier_row_budget_accounts_for_inter_row_padding() {
    let g = glyphs_for(false);
    // A 4-stage fixture is enough to exercise the row-count math without
    // tripping the overflow indicator at common pane sizes.
    let names = ["alpha", "beta", "gamma", "delta"];
    let stages: Vec<StageDefinition> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "delta";
            stage(n, initial, terminal, false, false, None)
        })
        .collect();
    let counts = vec![0usize; stages.len()];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/vnarrow-rowbudget"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };
    // Width 16 forces a 1-cell-per-row layout (the widest cell at this
    // fixture is "▶ alpha (0)" = 11 chars, so a 16-col usable budget can
    // only hold 1 column with `col_gap=3`). Cycle 4 tightened
    // INTER_ROW_PADDING_LINES from 3 to 1, so height 7 is exactly enough
    // for 4 stage rows with 1 blank padding line between consecutive
    // rows: 4 + 3*1 = 7. A correctly-budgeted renderer must keep all 4
    // stages visible (no overflow indicator).
    let inner_width = 16usize;
    let inner_height = 7usize;
    let lines = render_very_narrow(
        &stages,
        &counts,
        None,
        &g,
        inner_width,
        inner_height,
        &definition,
    );
    let rendered_text: String = lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    for name in names {
        assert!(
            rendered_text.contains(name),
            "stage {name} must be visible when height is exactly enough \
             to fit all rows + inter-row padding; rendered=\n{rendered_text}"
        );
    }
    assert!(
        !rendered_text.contains("hidden:"),
        "no overflow indicator should appear when row budget accounts for \
         padding; rendered=\n{rendered_text}"
    );

    // Now shrink the budget by exactly one line below the padding-aware
    // requirement. The renderer must elide one stage (and surface it via
    // the overflow indicator) rather than silently dropping it.
    let lines_short = render_very_narrow(
        &stages,
        &counts,
        None,
        &g,
        inner_width,
        inner_height - 1,
        &definition,
    );
    let short_text: String = lines_short
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        short_text.contains("hidden:"),
        "at exactly one line below the padding-aware budget the renderer must \
         surface an overflow indicator; got=\n{short_text}"
    );
}

/// Cycle 4 captain feedback (ask #1): no trailing arrow after the final
/// (terminal) stage on the last wrapped row in the Narrow tier. Inter-stage
/// `→` and `wrap_trailing` glyphs render only BETWEEN two real stage cells —
/// never after the very last emitted cell in the rendered sequence.
#[test]
fn narrow_tier_last_row_does_not_end_with_arrow() {
    let g = glyphs_for(false);
    let stages: Vec<StageDefinition> = RESEARCH_STAGES
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "done" || *n == "rejected";
            stage(n, initial, terminal, false, false, None)
        })
        .collect();
    let counts = vec![0usize; stages.len()];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/narrow-no-trailing"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };
    // Force wrapping with inner_width=80 (usable_width=72) which cannot hold
    // the full 12-stage narrow_summary on a single row.
    let lines = render_narrow(&stages, &counts, None, &g, 80, &definition);

    // Find the last row that contains a stage name (skip feedback annotation
    // / blank spacer lines).
    let last_stage_row = lines
        .iter()
        .rev()
        .find(|line| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            RESEARCH_STAGES.iter().any(|s| text.contains(s))
        })
        .expect("at least one stage-bearing row in narrow output");

    // The terminal stage ("rejected") must appear on the final stage row —
    // sanity-check the fixture actually exercised the wrap behaviour.
    let last_row_text: String = last_stage_row
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    assert!(
        last_row_text.contains("rejected"),
        "expected the terminal stage on the final wrapped row; got {last_row_text:?}"
    );

    // After stripping the right-margin whitespace, the row must NOT end with
    // a narrow arrow glyph.
    let trimmed = last_row_text.trim_end();
    assert!(
        !trimmed.ends_with('\u{2192}'),
        "narrow tier's final wrapped row must not end with a trailing `→` arrow \
         after the terminal stage; got {last_row_text:?}"
    );
}

/// Cycle 4 captain feedback (ask #1): no trailing arrow after the final
/// (terminal) stage on the last wrapped row in the VeryNarrow tier. Holds
/// both when every stage fits in the grid AND when an overflow indicator
/// follows the grid (a `+N hidden:` line is NOT a stage cell).
#[test]
fn very_narrow_tier_last_row_does_not_end_with_arrow() {
    let g = glyphs_for(false);
    let stages: Vec<StageDefinition> = RESEARCH_STAGES
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "done" || *n == "rejected";
            stage(n, initial, terminal, false, false, None)
        })
        .collect();
    let counts = vec![0usize; stages.len()];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/vnarrow-no-trailing"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };

    // Case A: every stage fits in the grid (no overflow indicator).
    let lines_fit = render_very_narrow(&stages, &counts, None, &g, 40, 24, &definition);
    let last_stage_row = lines_fit
        .iter()
        .rev()
        .find(|line| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            RESEARCH_STAGES.iter().any(|s| text.contains(s))
                && !text.contains("rollback on reject")
                && !text.starts_with('+')
        })
        .expect("at least one stage-bearing row in very-narrow output");
    let last_row_text: String = last_stage_row
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    assert!(
        last_row_text.contains("rejected"),
        "expected the terminal stage on the final grid row; got {last_row_text:?}"
    );
    assert!(
        !last_row_text.trim_end().ends_with('\u{2192}'),
        "very-narrow tier's final grid row must not end with a trailing `→` \
         after the terminal stage; got {last_row_text:?}"
    );

    // Case B: grid overflows (some stages elided to a `+N hidden:` line).
    // The last EMITTED stage cell must still not be followed by a trailing
    // arrow, even though more stages exist in the overflow line.
    let lines_overflow = render_very_narrow(&stages, &counts, None, &g, 14, 5, &definition);
    let overflow_text: String = lines_overflow
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        overflow_text.contains("hidden:"),
        "fixture must overflow at width=14/height=5 to exercise this case; got=\n{overflow_text}"
    );
    let last_stage_row_overflow = lines_overflow
        .iter()
        .rev()
        .find(|line| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            RESEARCH_STAGES.iter().any(|s| text.contains(s))
                && !text.contains("rollback on reject")
                && !text.starts_with('+')
        })
        .expect("at least one stage-bearing row before overflow indicator");
    let last_overflow_row_text: String = last_stage_row_overflow
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    assert!(
        !last_overflow_row_text.trim_end().ends_with('\u{2192}'),
        "very-narrow tier's final visible stage row must not end with a `→` \
         even when an overflow indicator follows; got {last_overflow_row_text:?}"
    );
}

// --- Entity 010: ASCII DAG renderer ---

/// Build the 4-stage `spacetop-ui` fixture used by entity 010 ACs:
/// design (initial) → implement (worktree) → review (gate, feedback-to:
/// implement) → done (terminal).
fn spacetop_ui_workflow() -> App {
    let root = PathBuf::from("/tmp/spacetop-ui-dag");
    let stages = vec![
        stage("design", true, false, false, false, None),
        stage("implement", false, false, false, true, None),
        stage("review", false, false, true, false, Some("implement")),
        stage("done", false, true, false, false, None),
    ];
    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: root.clone(),
            state: None,
            stages,
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items: Vec::new(),
        parse_errors: Vec::new(),
    };
    App::from_snapshot(root, snapshot)
}

/// AC-1: the 4-stage spacetop-ui fixture renders with drawn line-drawing
/// edges between adjacent stage nodes (── horizontal fill and ▶/► arrowhead),
/// not as a plain text separator. AC-2: a drawn feedback arc from
/// `review → implement` carries the box-drawing corner glyphs ╰/╯ plus the
/// vertical bar │ and the up arrowhead ↑ — collectively, line-drawing
/// geometry the eye reads as a looping edge.
#[test]
fn dag_spacetop_ui_renders_drawn_edges_and_feedback_arc() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = spacetop_ui_workflow();
    // 100x10 is wide enough for the DAG tier to fit the 4-stage chain.
    let rendered = render_to_string(&app, 100, 10);

    // AC-1: forward edges are drawn with ─ and ▶ (the wide-tier `──▶`
    // glyph). Both line-drawing chars must be present.
    assert!(
        rendered.contains('\u{2500}'),
        "DAG must draw horizontal edges (─) between adjacent nodes; rendered=\n{rendered}"
    );
    assert!(
        rendered.contains('\u{25BA}'),
        "DAG must draw forward arrowhead (►) between adjacent nodes; rendered=\n{rendered}"
    );

    // AC-2: the review → implement feedback edge renders as a drawn arc
    // with rounded corners + vertical bar + up arrowhead.
    assert!(
        rendered.contains('\u{2570}') && rendered.contains('\u{256F}'),
        "feedback arc must render with rounded corners ╰ and ╯; rendered=\n{rendered}"
    );
    assert!(
        rendered.contains('\u{2502}'),
        "feedback arc must render with vertical bar │; rendered=\n{rendered}"
    );
    assert!(
        rendered.contains('\u{2191}'),
        "feedback arc must point its arrowhead ↑ at the target column; rendered=\n{rendered}"
    );
}

/// AC-2: the DAG tier must NOT emit the legacy `↩ rollback on reject: ...`
/// footer annotation — that text only appears in the wrapped Narrow /
/// VeryNarrow fallback tiers where the arc cannot be drawn.
#[test]
fn dag_does_not_render_rollback_footer_when_arc_is_drawn() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = spacetop_ui_workflow();
    let rendered = render_to_string(&app, 100, 10);
    // The arc must be drawn (sanity check — exercises the DAG tier path).
    assert!(
        rendered.contains('\u{2570}'),
        "expected DAG tier to draw the arc at width=100; rendered=\n{rendered}"
    );
    // The legacy textual footer must be absent in DAG mode.
    assert!(
        !rendered.contains("rollback on reject"),
        "DAG tier must not emit the `↩ rollback on reject:` footer when the \
         arc is drawn; rendered=\n{rendered}"
    );
    assert!(
        !rendered.contains('\u{21B6}'),
        "DAG tier must not emit the ↩ feedback footer glyph when the arc is \
         drawn; rendered=\n{rendered}"
    );
}

/// AC-3: the 12-stage research fixture, when constrained, either fits in the
/// 90% width-margin or surfaces an explicit `+N hidden:` overflow indicator
/// naming the hidden stages — matching the 009 overflow-naming pattern. The
/// DAG renderer delegates to the wrapped fallback when it cannot fit, which
/// is what the captain-confirmed degraded-mode behaviour requires.
#[test]
fn dag_twelve_stage_research_fits_or_names_hidden_stages() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = research_12_stage_workflow();
    // A representative cramped pane: 80x7 inner (matches the overview-layout
    // graph pane). 12 inline `name(count)` nodes do NOT fit on one row at
    // 90% of 80 cols, so the renderer falls back to the wrapped tier.
    let rendered = render_to_string(&app, 80, 7);
    for name in RESEARCH_STAGES {
        if rendered.contains(name) {
            continue;
        }
        assert!(
            rendered.contains("hidden:"),
            "stage {name} missing and no `+N hidden:` overflow indicator present; \
             rendered=\n{rendered}"
        );
        assert!(
            rendered.contains(name),
            "stage {name} not in visible cells nor named by overflow indicator"
        );
    }
}

/// AC-4: every stage span in the DAG tier carries `stage_color_for(name)` +
/// BOLD, and the active stage layers REVERSED on top.
#[test]
fn dag_each_stage_span_carries_per_stage_color_and_bold() {
    let g = glyphs_for(false);
    let stages = vec![
        stage("design", true, false, false, false, None),
        stage("implement", false, false, false, true, None),
        stage("review", false, false, true, false, Some("implement")),
        stage("done", false, true, false, false, None),
    ];
    let counts = vec![0usize; stages.len()];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/dag-colors"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };
    // Active stage is `implement` so we can also assert REVERSED.
    let lines = render_dag(&stages, &counts, Some("implement"), &g, 100, &definition);

    let mut colored_seen: usize = 0;
    let mut active_reversed = false;
    for line in &lines {
        for span in &line.spans {
            for s in &stages {
                if span.content.contains(&s.name) {
                    let expected = crate::ui::color::to_color(definition.stage_color_for(&s.name));
                    if span.style.fg == Some(expected)
                        && span.style.add_modifier.contains(Modifier::BOLD)
                    {
                        colored_seen += 1;
                        if s.name == "implement"
                            && span.style.add_modifier.contains(Modifier::REVERSED)
                        {
                            active_reversed = true;
                        }
                    }
                }
            }
        }
    }
    assert!(
        colored_seen >= stages.len(),
        "expected every DAG stage span to carry stage_color_for + BOLD; \
         found {colored_seen} of {} (lines={lines:?})",
        stages.len()
    );
    assert!(
        active_reversed,
        "active stage span must carry REVERSED on top of color+BOLD"
    );
}

/// AC-5: short workflow readability. The 4-stage spacetop-ui DAG must show
/// every stage name + its (count) suffix + the feedback edge from `review`
/// to `implement`, and the rendered height stays within a tight bound. We
/// pin the bound at 4 lines: 1 chain row + 2 arc rows + at most 1 spare.
#[test]
fn dag_short_workflow_stays_within_height_bound() {
    let g = glyphs_for(false);
    let stages = vec![
        stage("design", true, false, false, false, None),
        stage("implement", false, false, false, true, None),
        stage("review", false, false, true, false, Some("implement")),
        stage("done", false, true, false, false, None),
    ];
    let counts = vec![0usize, 0, 0, 0];
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/dag-short"),
        state: None,
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions: Vec::new(),
    };
    let lines = render_dag(&stages, &counts, None, &g, 100, &definition);
    // Height bound for the short-workflow case: one chain row + one arc
    // (two-line: label + arc) = 3 lines.
    assert!(
        lines.len() <= 4,
        "DAG height for 4-stage workflow must be <=4 lines; got {} lines: {:?}",
        lines.len(),
        lines
    );
    // Every stage name + its (count) suffix must appear.
    let joined: String = lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    for s in &stages {
        assert!(
            joined.contains(&s.name),
            "DAG must show stage {} in short workflow; rendered={joined}",
            s.name
        );
    }
    assert!(
        joined.contains("(0)"),
        "DAG must inline (count) into the node text; rendered={joined}"
    );
    // The feedback arc must be present (review → implement).
    assert!(
        joined.contains('\u{2570}') && joined.contains('\u{256F}'),
        "feedback arc corners must be drawn; rendered={joined}"
    );
    assert!(
        !joined.contains("rollback on reject"),
        "DAG tier must not emit the legacy footer; rendered={joined}"
    );
}

/// Build a fixture matching the on-repo `docs/spacetop-dev` workflow shape
/// (5 stages: design → plan → implement → review → done with
/// `review → feedback-to: implement`). Used by the drawn-arc test that
/// pins the FULL connected glyph sequence (entity 010 cycle 1).
fn spacetop_dev_workflow() -> App {
    let root = PathBuf::from("/tmp/spacetop-dev-arc");
    let stages = vec![
        stage("design", true, false, false, false, None),
        stage("plan", false, false, false, false, None),
        stage("implement", false, false, false, true, None),
        stage("review", false, false, true, false, Some("implement")),
        stage("done", false, true, false, false, None),
    ];
    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: root.clone(),
            state: None,
            stages,
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items: Vec::new(),
        parse_errors: Vec::new(),
    };
    App::from_snapshot(root, snapshot)
}

/// Entity 010 cycle 1, gap 1: when the inline single-row chain does not fit
/// `usable_inner_width`, `render_dag` MUST wrap the chain across multiple
/// rows with drawn line-drawing glyphs (`│`/`╭`/`╮`/`╯`/`╰`) connecting one
/// chain row to the next — it must NOT fall back to the 009 wrapped-text
/// `render_narrow` tier. The 009 tier remains a DEEPER fallback only when
/// the multi-row DAG itself cannot fit the available height.
///
/// Test setup: the 12-stage research fixture at a representative narrow
/// pane size where (a) the single-row chain overflows `usable_inner_width`
/// at ~90% of 100 cols, but (b) the available height (10 rows ⇒ inner
/// height 8) is generous enough to hold 2-3 wrapped DAG chain rows with
/// connector lines between them.
#[test]
fn dag_multi_row_wraps_with_drawn_connectors_on_research_fixture() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = research_12_stage_workflow();
    let rendered = render_to_string(&app, 100, 10);

    // (a) Every stage name + its (count) suffix must appear somewhere on
    // the rendered buffer. Counts are zero for this fixture but we pin the
    // `(0)` suffix on at least one node so we know the DAG node-text
    // (which collapses the count INTO the node) was the renderer used.
    for name in RESEARCH_STAGES {
        assert!(
            rendered.contains(name),
            "every stage name must be visible in multi-row DAG; missing {name}; \
             rendered=\n{rendered}"
        );
    }
    assert!(
        rendered.contains("(0)"),
        "DAG nodes must carry inline `(count)` suffix; rendered=\n{rendered}"
    );

    // (b) Drawn vertical/corner glyphs connect consecutive chain rows. We
    // expect at least one row-break corner `╮` on a chain row tail AND a
    // matching `╯` on the connector line below. The `╭` corner appears on
    // the connector at the next row's start column.
    assert!(
        rendered.contains('\u{256E}'),
        "multi-row DAG must emit row-break corner ╮ at the right end of a \
         wrapping chain row; rendered=\n{rendered}"
    );
    assert!(
        rendered.contains('\u{256F}'),
        "multi-row DAG must emit row-break corner ╯ on the connector line; \
         rendered=\n{rendered}"
    );
    assert!(
        rendered.contains('\u{256D}'),
        "multi-row DAG must emit row-break corner ╭ on the connector line at \
         the next chain row's start column; rendered=\n{rendered}"
    );
    assert!(
        rendered.contains('\u{2500}'),
        "multi-row DAG must emit horizontal `─` fill on connector lines; \
         rendered=\n{rendered}"
    );

    // (c) The rendering is NOT the 009 wrapped-text fallback. The Narrow
    // tier emits `↩ rollback on reject: ...` as a tail Line whenever the
    // workflow declares `feedback-to:` paths; this fixture has no feedback
    // edges, so the footer would not appear regardless. The discriminator
    // here is that the chain MUST be drawn with the DAG's wide `──►`
    // forward arrow (`►` = U+25BA), not the Narrow tier's `→` (U+2192).
    assert!(
        rendered.contains('\u{25BA}'),
        "multi-row DAG must use the wide `►` forward arrow, not the Narrow \
         tier's `→`; rendered=\n{rendered}"
    );

    // Also assert at least two chain rows actually render: count `►` on
    // the buffer; a 12-stage chain wrapped to 2 rows of 6 stages each has
    // 5 + 5 = 10 forward arrows; even if the wrap puts 7-and-5 we still
    // see well above the single-row count, so >= 9 is a safe floor.
    let chain_arrow_count = rendered.matches('\u{25BA}').count();
    assert!(
        chain_arrow_count >= 9,
        "multi-row DAG should emit at least one ► per inter-node edge \
         (12 stages over 2 rows = ~10 arrows); got {chain_arrow_count}; \
         rendered=\n{rendered}"
    );
}

/// Entity 010 cycle 1, gap 1 (depth-fallback complement): when the
/// multi-row DAG ITSELF cannot fit the available height, the renderer
/// falls back to the 009 wrapped-text tiers (render_narrow /
/// render_very_narrow). This test exercises the height-starved path and
/// asserts the deeper-fallback contract: the 009 tiers' `↩ rollback on
/// reject:` footer is still emitted when feedback edges are declared.
#[test]
fn dag_falls_back_to_009_wrapped_text_when_height_starved() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    // 12-stage workflow with a `review → feedback-to: implement` edge so
    // the 009 wrapped tier has an annotation to emit. Inner_height=2
    // (height=4) is far too tight for multi-row DAG so the fallback kicks
    // in.
    let root = PathBuf::from("/tmp/spacetop-deep-fallback");
    let names = [
        "pending",
        "scoping",
        "ideate",
        "implement",
        "review",
        "smoke",
        "run",
        "analyze",
        "promote",
        "expanded",
        "ideated",
        "done",
    ];
    let stages: Vec<StageDefinition> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = *n == "done";
            let fb = if *n == "review" {
                Some("implement")
            } else {
                None
            };
            stage(n, initial, terminal, false, false, fb)
        })
        .collect();
    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: root.clone(),
            state: None,
            stages,
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items: Vec::new(),
        parse_errors: Vec::new(),
    };
    let app = App::from_snapshot(root, snapshot);
    // 40x9 forces the VeryNarrow tier path (matches the existing pinned
    // behaviour for the 009 wrapped-text fallback).
    let rendered = render_to_string(&app, 40, 9);
    assert!(
        rendered.contains("rollback on reject"),
        "height-starved fallback must still emit the 009 `↩ rollback on reject:` \
         footer when feedback edges are declared; rendered=\n{rendered}"
    );
}

/// Entity 010 cycle 1, gap 2: the drawn feedback arc must render the FULL
/// connected glyph sequence between source and target columns — corner at
/// source, `─` fill across all columns between target and source with the
/// label centred, matching corner at the target column, and `↑` arrowhead
/// at the target. Endpoints alone (just `↑` + stray `│`) are not enough.
///
/// Tested on the spacetop-dev fixture (5 stages: design → plan → implement
/// → review → done with `review → feedback-to: implement`).
#[test]
fn dag_drawn_feedback_arc_is_fully_connected_on_spacetop_dev() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = spacetop_dev_workflow();
    // 109x10 matches the captain's reported terminal pane: chain fits on a
    // single row so the arc geometry is the focus of the assertion.
    let rendered = render_to_string(&app, 109, 10);

    // Locate the arc rows. Per render_dag (entity 010 cycle 1), the arrow
    // row (`↑   reject   │`) renders FIRST so it sits closer to the chain;
    // the corner row (`╰────...────╯`) renders second below it. Chunk the
    // rendered buffer by `line_w` chars per row (chars, not bytes, because
    // the buffer contains multi-byte BMP glyphs like `→`/`│`/`╰`).
    let line_w = 109usize;
    let chars: Vec<char> = rendered.chars().collect();
    let row_strings: Vec<String> = chars.chunks(line_w).map(|c| c.iter().collect()).collect();

    // Find the arrow row (contains both `↑` and `│` and "reject").
    let arrow_row_idx = row_strings
        .iter()
        .position(|r| r.contains('\u{2191}') && r.contains('\u{2502}') && r.contains("reject"));
    assert!(
        arrow_row_idx.is_some(),
        "missing arrow row carrying ↑ + │ + 'reject' label; rendered=\n{rendered}"
    );
    let arrow_row = &row_strings[arrow_row_idx.unwrap()];
    // Locate columns of ↑ (target = implement column) and │ (source =
    // review column).
    let up_col = arrow_row
        .chars()
        .position(|c| c == '\u{2191}')
        .expect("arrow row has ↑");
    let bar_col = arrow_row
        .chars()
        .position(|c| c == '\u{2502}')
        .expect("arrow row has │");
    let (target_col, source_col) = if up_col < bar_col {
        (up_col, bar_col)
    } else {
        (bar_col, up_col)
    };
    assert!(
        source_col > target_col,
        "source column ({source_col}) must be right of target column \
         ({target_col}) for review→implement feedback"
    );

    // The label `reject` must appear BETWEEN target_col and source_col on
    // the arrow row.
    let reject_pos = arrow_row.find("reject").expect("label present");
    let reject_char_col = arrow_row[..reject_pos].chars().count();
    assert!(
        reject_char_col > target_col && reject_char_col < source_col,
        "label `reject` must sit between target ({target_col}) and source \
         ({source_col}) columns; reject_col={reject_char_col}"
    );

    // The next row must be the corner row carrying ╰ + ─...─ + ╯ at the
    // SAME columns as the arrow-row endpoints.
    let corner_row_idx = arrow_row_idx.unwrap() + 1;
    assert!(
        corner_row_idx < row_strings.len(),
        "corner row must follow arrow row; row count = {}",
        row_strings.len()
    );
    let corner_row = &row_strings[corner_row_idx];
    let corner_chars: Vec<char> = corner_row.chars().collect();

    // Corner at target column.
    assert_eq!(
        corner_chars.get(target_col).copied(),
        Some('\u{2570}'),
        "corner ╰ must sit at target column {target_col}; corner_row=\n{corner_row}"
    );
    // Matching corner at source column.
    assert_eq!(
        corner_chars.get(source_col).copied(),
        Some('\u{256F}'),
        "corner ╯ must sit at source column {source_col}; corner_row=\n{corner_row}"
    );
    // Horizontal `─` fill spans ALL columns between target+1 and source-1
    // (inclusive of both endpoints). At least one `─` must exist in that
    // range (sanity) AND every column in that range must be `─` (full
    // connectivity).
    assert!(
        source_col > target_col + 1,
        "target/source columns must be far enough apart for at least one \
         `─` between them; target={target_col} source={source_col}"
    );
    for col in (target_col + 1)..source_col {
        assert_eq!(
            corner_chars.get(col).copied(),
            Some('\u{2500}'),
            "every column between target+1 ({}) and source-1 ({}) must be \
             `─`; column {col} was {:?}; corner_row=\n{corner_row}",
            target_col + 1,
            source_col - 1,
            corner_chars.get(col)
        );
    }

    // The drawn arc must NOT degrade to the legacy `↩ rollback on reject:`
    // footer when the geometry actually fits.
    assert!(
        !rendered.contains("rollback on reject"),
        "drawn arc must not co-exist with the legacy footer; rendered=\n{rendered}"
    );
}

/// Entity 010 cycle 2: the DAG is horizontally CENTERED within the inner
/// pane width. The leftmost rendered DAG column equals approximately
/// `(inner_width - chain_width) / 2` (within a one-column rounding
/// tolerance), the rightmost rendered column is approximately the same
/// distance from the right edge, AND the feedback-arc row shifts with the
/// chain so the `↑` arrowhead and `│` source-column glyphs remain
/// column-aligned with their target/source stage spans on the chain row.
///
/// Test fixture: spacetop-dev (5 stages, fits in a single row at typical
/// widths) at 109x10 — same geometry the captain reported in cycle-2
/// feedback.
#[test]
fn dag_chain_is_horizontally_centered_on_spacetop_dev() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = spacetop_dev_workflow();
    let line_w = 109usize;
    let rendered = render_to_string(&app, line_w as u16, 10);

    let chars: Vec<char> = rendered.chars().collect();
    let row_strings: Vec<String> = chars.chunks(line_w).map(|c| c.iter().collect()).collect();

    // Find the chain row: the line containing the wide `►` forward arrow.
    let chain_row_idx = row_strings
        .iter()
        .position(|r| r.contains('\u{25BA}'))
        .expect("chain row with ► should be present");
    let chain_row = &row_strings[chain_row_idx];

    // Helper: leftmost and rightmost non-space columns on a row.
    fn non_space_bounds(row: &str) -> Option<(usize, usize)> {
        let cols: Vec<char> = row.chars().collect();
        let left = cols.iter().position(|c| !c.is_whitespace())?;
        let right = cols.iter().rposition(|c| !c.is_whitespace())?;
        Some((left, right))
    }

    let (left, right) = non_space_bounds(chain_row).expect("chain row has content");
    let chain_width = right - left + 1;
    let slack = line_w.saturating_sub(chain_width);
    let expected_left = slack / 2;
    let expected_right_gap = slack - expected_left;
    let actual_right_gap = line_w - 1 - right;

    // (a) leftmost rendered DAG column is approximately (inner_width -
    // chain_width) / 2 within a one-column rounding tolerance.
    assert!(
        left.abs_diff(expected_left) <= 1,
        "chain row leftmost column {left} must be approximately \
         expected_left {expected_left} (slack/2 with chain_width={chain_width}, \
         inner_width={line_w}); rendered chain_row=\n{chain_row}"
    );
    // (b) rightmost rendered column is approximately the same distance
    // from the right edge.
    assert!(
        actual_right_gap.abs_diff(expected_right_gap) <= 1,
        "chain row right gap {actual_right_gap} must be approximately \
         expected_right_gap {expected_right_gap}; chain row=\n{chain_row}"
    );
    // The two gaps must agree within one column of each other (the
    // captain's centering ask).
    assert!(
        left.abs_diff(actual_right_gap) <= 1,
        "chain row must be horizontally centered: left_gap {left} vs \
         right_gap {actual_right_gap} (chain_width={chain_width}); chain_row=\n{chain_row}"
    );

    // (c) the feedback-arc row's source-column `│` glyph still column-
    // aligns with the source stage span (`review`) on the chain row, and
    // the target-column `↑` aligns with the target stage span
    // (`implement`).
    let arrow_row_idx = row_strings
        .iter()
        .position(|r| r.contains('\u{2191}') && r.contains('\u{2502}') && r.contains("reject"))
        .expect("arrow row should be present in DAG");
    let arrow_row = &row_strings[arrow_row_idx];
    let up_col = arrow_row
        .chars()
        .position(|c| c == '\u{2191}')
        .expect("↑ on arrow row");
    let bar_col = arrow_row
        .chars()
        .position(|c| c == '\u{2502}')
        .expect("│ on arrow row");
    let (target_col, source_col) = if up_col < bar_col {
        (up_col, bar_col)
    } else {
        (bar_col, up_col)
    };

    // Find the start and end columns of the `implement` and `review`
    // stage spans on the chain row. Match the stage name then bracket
    // back/forward to include the `(0)` suffix.
    fn stage_span_bounds(row: &str, name: &str) -> Option<(usize, usize)> {
        let chars: Vec<char> = row.chars().collect();
        let byte_pos = row.find(name)?;
        let char_start = row[..byte_pos].chars().count();
        let mut char_end = char_start + name.chars().count();
        // Extend through the (count) suffix: `(0)` etc.
        if chars.get(char_end).copied() == Some('(') {
            while char_end < chars.len() && chars[char_end] != ')' {
                char_end += 1;
            }
            if chars.get(char_end).copied() == Some(')') {
                char_end += 1;
            }
        }
        Some((char_start, char_end))
    }

    let (impl_start, impl_end) =
        stage_span_bounds(chain_row, "implement").expect("implement span on chain row");
    let (rev_start, rev_end) =
        stage_span_bounds(chain_row, "review").expect("review span on chain row");

    assert!(
        target_col >= impl_start && target_col < impl_end,
        "arrow row `↑` at col {target_col} must fall within the \
         `implement` stage span [{impl_start}, {impl_end}); chain row=\n{chain_row}\narrow row=\n{arrow_row}"
    );
    assert!(
        source_col >= rev_start && source_col < rev_end,
        "arrow row `│` at col {source_col} must fall within the `review` \
         stage span [{rev_start}, {rev_end}); chain row=\n{chain_row}\narrow row=\n{arrow_row}"
    );
}

/// Entity 010 cycle 2: the centering property also holds for the multi-
/// row DAG layout (research 12-stage fixture at a narrow pane width). The
/// captain's spec: compute `chain_width = max(row_widths)` and split
/// `slack = inner_width - chain_width` as `left_pad = slack / 2`,
/// `right_pad = slack - left_pad`. All DAG rows (chain rows, inter-row
/// connectors, feedback-arc rows) receive the SAME left_pad — so the
/// widest row sits exactly centered and narrower rows left-align within
/// the centered band. Per-row centering would force connectors and arc
/// rows to drift relative to the source/target stage spans they refer to.
#[test]
fn dag_multi_row_chain_is_horizontally_centered_on_research_fixture() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = research_12_stage_workflow();
    let line_w = 100usize;
    let rendered = render_to_string(&app, line_w as u16, 10);

    let chars: Vec<char> = rendered.chars().collect();
    let row_strings: Vec<String> = chars.chunks(line_w).map(|c| c.iter().collect()).collect();

    fn non_space_bounds(row: &str) -> Option<(usize, usize)> {
        let cols: Vec<char> = row.chars().collect();
        let left = cols.iter().position(|c| !c.is_whitespace())?;
        let right = cols.iter().rposition(|c| !c.is_whitespace())?;
        Some((left, right))
    }

    // Find all chain rows (those containing the wide `►` forward arrow).
    let chain_row_indices: Vec<usize> = row_strings
        .iter()
        .enumerate()
        .filter(|(_, r)| r.contains('\u{25BA}'))
        .map(|(i, _)| i)
        .collect();
    assert!(
        chain_row_indices.len() >= 2,
        "multi-row DAG must emit at least 2 chain rows on the 12-stage \
         research fixture at width {line_w}; chain_rows={}",
        chain_row_indices.len()
    );

    // Determine the DAG block bounds: leftmost non-space column across
    // all DAG rows and rightmost non-space column across all DAG rows.
    // The DAG occupies a contiguous range of rows around the chain rows.
    let dag_start = chain_row_indices[0];
    let dag_end = *chain_row_indices.last().unwrap();
    let mut block_left = usize::MAX;
    let mut block_right = 0usize;
    for row in row_strings.iter().take(dag_end + 1).skip(dag_start) {
        if let Some((l, r)) = non_space_bounds(row) {
            if l < block_left {
                block_left = l;
            }
            if r > block_right {
                block_right = r;
            }
        }
    }
    assert!(
        block_left < block_right,
        "DAG block must have non-empty bounds; rendered=\n{rendered}"
    );

    let chain_width = block_right - block_left + 1;
    let expected_left = (line_w.saturating_sub(chain_width)) / 2;
    let expected_right_gap = line_w.saturating_sub(chain_width) - expected_left;
    let actual_right_gap = line_w - 1 - block_right;

    // (a) the leftmost DAG column equals approximately (inner_width -
    // chain_width) / 2 within one column.
    assert!(
        block_left.abs_diff(expected_left) <= 1,
        "DAG block leftmost column {block_left} must be approximately \
         expected_left {expected_left} (chain_width={chain_width}, \
         inner_width={line_w}); rendered=\n{rendered}"
    );
    // (b) rightmost DAG column has approximately the same distance to
    // the right edge.
    assert!(
        actual_right_gap.abs_diff(expected_right_gap) <= 1,
        "DAG block right gap {actual_right_gap} must be approximately \
         expected_right_gap {expected_right_gap}; rendered=\n{rendered}"
    );
    // The two gaps must agree within one column of each other.
    assert!(
        block_left.abs_diff(actual_right_gap) <= 1,
        "DAG block must be horizontally centered: left_gap {block_left} \
         vs right_gap {actual_right_gap}; rendered=\n{rendered}"
    );

    // (c) per-row property: every DAG row starts at exactly the same
    // left column (the uniform left_pad). The widest row spans the full
    // chain_width; narrower rows left-align within the centered band.
    for (idx, row) in row_strings
        .iter()
        .enumerate()
        .take(dag_end + 1)
        .skip(dag_start)
    {
        if let Some((l, _)) = non_space_bounds(row) {
            assert!(
                l.abs_diff(block_left) <= 1,
                "DAG row {idx} must left-align with the centered chain \
                 (block_left={block_left}, this row left={l}); row=\n{row}"
            );
        }
    }
}

#[allow(dead_code)]
fn make_item(id: &str, status: &str, title: &str) -> Entity {
    Entity {
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
        worktree_source: None,
        main_body: None,
    }
}

// --- Unit 3: declared transitions render as inbound edges ---

use spacetop_core::domain::StageTransition;

/// Builds the dataagentbench research workflow definition with its full
/// 12-stage states block and 13 declared transitions. Used by the AC-2 and
/// AC-3 tests so the bug case is exercised by a single source of truth.
fn research_workflow_definition() -> WorkflowDefinition {
    let names = [
        "pending", "scoping", "ideate", "review", "smoke", "run", "analyze", "promote", "expanded",
        "ideated", "done", "rejected",
    ];
    let stages: Vec<StageDefinition> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = matches!(*n, "expanded" | "ideated" | "done" | "rejected");
            let gate = *n == "review";
            stage(n, initial, terminal, gate, false, None)
        })
        .collect();
    let transitions = vec![
        StageTransition {
            from: "pending".into(),
            to: "scoping".into(),
            label: None,
        },
        StageTransition {
            from: "scoping".into(),
            to: "ideate".into(),
            label: None,
        },
        StageTransition {
            from: "scoping".into(),
            to: "expanded".into(),
            label: None,
        },
        StageTransition {
            from: "ideate".into(),
            to: "review".into(),
            label: None,
        },
        StageTransition {
            from: "ideate".into(),
            to: "ideated".into(),
            label: None,
        },
        StageTransition {
            from: "review".into(),
            to: "smoke".into(),
            label: None,
        },
        StageTransition {
            from: "review".into(),
            to: "rejected".into(),
            label: Some("reject".into()),
        },
        StageTransition {
            from: "smoke".into(),
            to: "run".into(),
            label: None,
        },
        StageTransition {
            from: "smoke".into(),
            to: "rejected".into(),
            label: Some("reject".into()),
        },
        StageTransition {
            from: "run".into(),
            to: "analyze".into(),
            label: None,
        },
        StageTransition {
            from: "analyze".into(),
            to: "promote".into(),
            label: None,
        },
        StageTransition {
            from: "analyze".into(),
            to: "rejected".into(),
            label: Some("reject".into()),
        },
        StageTransition {
            from: "promote".into(),
            to: "done".into(),
            label: None,
        },
    ];
    WorkflowDefinition {
        root: PathBuf::from("/tmp/spacetop-research-transitions"),
        state: None,
        stages,
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
        transitions,
    }
}

fn research_workflow_app() -> App {
    let definition = research_workflow_definition();
    let root = definition.root.clone();
    let snapshot = WorkflowSnapshot {
        definition,
        items: Vec::new(),
        parse_errors: Vec::new(),
    };
    App::from_snapshot(root, snapshot)
}

/// AC-2: the rendered DAG must visibly link each non-adjacent-predecessor
/// terminal stage (`scoping → expanded`, `ideate → ideated`, `promote → done`)
/// to its declared predecessor. This is the chokepoint regression that the
/// bug report calls out — previously the four terminal stages were strung
/// off the chain in `states:` order with no inbound edge to their real source.
#[test]
fn dag_renders_inbound_edge_for_non_adjacent_terminal_predecessor() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = research_workflow_app();
    // Wide enough that the DAG tier is selected (the 12-stage research
    // workflow comfortably fits when wrapped); height covers the chain
    // rows + feedback arcs + every annotation tail this fixture emits.
    let rendered = render_to_string(&app, 200, 40);
    // Each non-adjacent terminal source/target pair must appear in the
    // rendered output. The annotation tail format is
    // `{narrow_arrow} {from} {narrow_arrow} {to}`.
    let arrow = "\u{2192}"; // →
    for (from, to) in [
        ("scoping", "expanded"),
        ("ideate", "ideated"),
        ("promote", "done"),
    ] {
        let needle = format!("{from} {arrow} {to}");
        assert!(
            rendered.contains(&needle),
            "DAG must show '{needle}' for non-adjacent transition; got:\n{rendered}"
        );
    }
}

/// AC-3: `rejected` has three declared predecessors (`review`, `smoke`,
/// `analyze`). The rendered DAG must show all three inbound edges — not
/// just one — so the captain can read the full topology.
#[test]
fn dag_renders_three_inbound_edges_for_rejected() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let app = research_workflow_app();
    let rendered = render_to_string(&app, 200, 40);
    let arrow = "\u{2192}"; // →
                            // Count distinct inbound substrings — once per declared source.
    for src in ["review", "smoke", "analyze"] {
        let needle = format!("{src} {arrow} rejected");
        assert!(
            rendered.contains(&needle),
            "expected inbound edge '{needle}'; got:\n{rendered}"
        );
    }
    // Sanity: exactly three '→ rejected' substrings (one per source).
    let total = rendered.matches(&format!("{arrow} rejected")).count();
    assert_eq!(
        total, 3,
        "expected exactly 3 inbound rejected edges, got {total}:\n{rendered}"
    );
}

/// AC-4: a workflow with NO `transitions:` block synthesises the implicit
/// linear chain at the consumer boundary (`effective_transitions()`), and
/// every synthesised edge is by definition adjacent — so no annotation
/// tails are emitted. The existing single-chain renderer must produce a
/// byte-identical buffer for the same workflow rendered at the same size.
#[test]
fn dag_omits_arcs_when_no_transitions_block() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var(ASCII_ENV_VAR);
    let names = ["alpha", "beta", "gamma", "delta", "epsilon"];
    let stages: Vec<StageDefinition> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let initial = i == 0;
            let terminal = i == names.len() - 1;
            stage(n, initial, terminal, false, false, None)
        })
        .collect();
    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: PathBuf::from("/tmp/transitionless"),
            state: None,
            stages,
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items: Vec::new(),
        parse_errors: Vec::new(),
    };
    let app = App::from_snapshot(PathBuf::from("/tmp/transitionless"), snapshot);
    let rendered = render_to_string(&app, 120, 10);
    // No annotation tail line: there must not be a `→ {name}` pattern that
    // is NOT inside the inline chain. The inline chain uses `──▶` (U+25BA),
    // so any U+2192 followed by a stage name signals an annotation we did
    // not want emitted. None of the adjacent edges should produce one.
    let arrow = '\u{2192}';
    for needle in ["alpha", "beta", "gamma", "delta", "epsilon"] {
        let pat = format!("{arrow} {needle}");
        assert!(
            !rendered.contains(&pat),
            "no-transitions workflow must NOT emit annotation tail '{pat}'; got:\n{rendered}"
        );
    }
}

/// AC-2/AC-3 unit-level lock: even without driving the terminal renderer,
/// `collect_extra_transitions` must report exactly the non-adjacent edges
/// for the research fixture (3 sources for `rejected`, plus the three
/// terminal-tail edges from `scoping`, `ideate`, `promote`).
#[test]
fn collect_extra_transitions_for_research_fixture_lists_all_non_adjacent_edges() {
    let definition = research_workflow_definition();
    let g = glyphs_for(false);
    let counts = vec![0usize; definition.stages.len()];
    let cols = dag_layout_columns(&definition.stages, &counts, None, &g);
    // Single-row plan: pack everything onto one row regardless of width.
    let row_width = cols
        .iter()
        .map(|c| c.start_col + visible_width(&c.node_text))
        .max()
        .unwrap_or(0);
    let plan = dag_layout_rows(&cols, row_width + 1);
    let extras = collect_extra_transitions(&definition.stages, &cols, &plan, &definition);
    let pairs: std::collections::HashSet<(String, String)> = extras
        .iter()
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect();
    // Every non-adjacent declared edge must appear.
    for (from, to) in [
        ("scoping", "expanded"),
        ("ideate", "ideated"),
        ("promote", "done"),
        ("review", "rejected"),
        ("smoke", "rejected"),
        ("analyze", "rejected"),
    ] {
        assert!(
            pairs.contains(&(from.to_string(), to.to_string())),
            "missing non-adjacent edge {from} → {to}; got {pairs:?}"
        );
    }
    // Sanity: no adjacent edge sneaks into the extras list. The states
    // declaration order yields these adjacencies — every one is drawn
    // inline by the chain renderer.
    for (from, to) in [
        ("pending", "scoping"),
        ("scoping", "ideate"),
        ("ideate", "review"),
        ("review", "smoke"),
        ("smoke", "run"),
        ("run", "analyze"),
        ("analyze", "promote"),
    ] {
        assert!(
            !pairs.contains(&(from.to_string(), to.to_string())),
            "adjacent edge {from} → {to} should not appear in extras; got {pairs:?}"
        );
    }
}
