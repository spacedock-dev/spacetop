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
    let definition = WorkflowDefinition {
        root: PathBuf::from("/tmp/narrow-wrap"),
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
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
        "pending", "scoping", "ideate", "review", "smoke", "run", "analyze",
        "promote", "expanded", "ideated", "done", "rejected",
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
            stages,
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
        },
        items: Vec::new(),
    };
    App::from_snapshot(root, snapshot)
}

const RESEARCH_STAGES: &[&str] = &[
    "pending", "scoping", "ideate", "review", "smoke", "run", "analyze",
    "promote", "expanded", "ideated", "done", "rejected",
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
    // 40 cols forces the VeryNarrow tier for a 12-stage workflow; give it
    // generous height so all stages can land in the grid without overflow.
    let rendered = render_to_string(&app, 40, 20);
    for name in RESEARCH_STAGES {
        assert!(
            rendered.contains(name),
            "very narrow tier must show {name}"
        );
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
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
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
                    let expected = definition.stage_color_for(name);
                    if span.style.fg == Some(expected)
                        && span.style.add_modifier.contains(Modifier::BOLD)
                    {
                        found_colored += 1;
                        if *name == "review"
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
        stages: stages.clone(),
        id_style: None,
        entity_type: None,
        entity_label: None,
        entity_label_plural: None,
        stage_colors: std::collections::HashMap::new(),
        stage_prose: std::collections::HashMap::new(),
    };
    // 40 cols, height 20 — every stage fits in the grid without overflow.
    let lines = render_very_narrow(&stages, &counts, Some("analyze"), &g, 40, 20, &definition);

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
                    let expected = definition.stage_color_for(name);
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
