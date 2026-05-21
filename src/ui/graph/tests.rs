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
            stage_prose: std::collections::HashMap::new(),
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
            stage_prose: std::collections::HashMap::new(),
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
        },
        items: Vec::new(),
    };
    let app = App::from_snapshot(PathBuf::from("/tmp/narrow-tier"), snapshot);
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
    let lines = render_narrow(&stages, &counts, None, &g);
    // Must produce at least 2 lines.
    assert!(
        lines.len() >= 2,
        "render_narrow must produce at least 2 lines for split DAG, got {}",
        lines.len()
    );
    // First row must contain the first-half stage names (alpha, beta, gamma).
    let row1: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        row1.contains("alpha"),
        "row 1 must contain 'alpha'; got: {row1:?}"
    );
    assert!(
        row1.contains("beta"),
        "row 1 must contain 'beta'; got: {row1:?}"
    );
    assert!(
        row1.contains("gamma"),
        "row 1 must contain 'gamma'; got: {row1:?}"
    );
    // Second row must contain the second-half stage names (delta, epsilon, done).
    let row2: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        row2.contains("delta"),
        "row 2 must contain 'delta'; got: {row2:?}"
    );
    assert!(
        row2.contains("epsilon"),
        "row 2 must contain 'epsilon'; got: {row2:?}"
    );
    assert!(
        row2.contains("done"),
        "row 2 must contain 'done'; got: {row2:?}"
    );
    // Row 1 must NOT contain second-half names (they must be on row 2).
    assert!(
        !row1.contains("delta"),
        "row 1 must not contain 'delta' (should be on row 2)"
    );
    assert!(
        !row1.contains("epsilon"),
        "row 1 must not contain 'epsilon' (should be on row 2)"
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
        worktree_source: None,
        main_body: None,
    }
}
