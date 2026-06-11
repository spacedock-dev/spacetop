use super::*;

#[test]
fn help_popup_toggles_with_question_mark_and_closes_on_esc() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
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
    assert!(rendered.contains("Esc"), "missing Esc binding");
    assert!(
        !rendered.contains("Esc / q"),
        "help should not claim q closes the help popup"
    );
    assert!(
        rendered.contains("PageUp         page list up"),
        "help should describe PageUp as list paging when preview is closed"
    );
    assert!(
        rendered.contains("PageDown       page list down"),
        "help should describe PageDown as list paging when preview is closed"
    );
    assert!(
        rendered.contains("press ? or Esc to close"),
        "missing close hint"
    );
}

/// AC-1 (task 041): the help popup lists the new `D` keybind that
/// opens the workflow-definition view.
#[test]
fn help_popup_lists_definition_keybind() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
    let mut app = App::load(root).expect("workflow should load");
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

    let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        rendered.contains("D              open workflow definition"),
        "help popup should list `D` binding; rendered=\n{rendered}"
    );
}

#[test]
fn help_popup_lists_p3_capability_view_keybinds() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
    let mut app = App::load(root).expect("workflow should load");
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

    let mut terminal = Terminal::new(TestBackend::new(160, 34)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(rendered.contains("/              search entities"));
    assert!(rendered.contains(":              open command palette"));
    assert!(rendered.contains("T              entity timeline (preview closed)"));
    assert!(rendered.contains("M              metrics view (preview closed)"));
    assert!(rendered.contains("A              activity feed (preview closed)"));
    assert!(rendered.contains("R              entity relations (preview closed)"));
}

#[test]
fn help_popup_renders_in_picker_mode() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spacetop_core::discovery::DiscoveredWorkflow;
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
    // The Overview block must render with no left/right margin gutter —
    // i.e. content starts at column 0 and the layout fills the terminal width.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
    let app = App::load(root).expect("workflow should load");
    let width: u16 = 200;
    let height: u16 = 30;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let buffer = terminal.backend().buffer();
    // Row 0 is the header bar; it starts with "Workflow" at col 0.
    let top_left = buffer[(0, 0)].symbol();
    assert_ne!(
        top_left, " ",
        "expected non-blank left edge of dashboard at (0,0), got blank"
    );
    // Row 1 is the graph pane's TOP border; it spans the full width.
    // The graph block uses TOP|BOTTOM borders only, so the top border character
    // at (0, 1) and (width-1, 1) should be non-blank.
    let graph_border_left = buffer[(0, 1)].symbol();
    let graph_border_right = buffer[(width - 1, 1)].symbol();
    assert_ne!(
        graph_border_left, " ",
        "expected non-blank left edge of graph pane border at (0,1), got blank"
    );
    assert_ne!(
        graph_border_right,
        " ",
        "expected non-blank right edge of graph pane border at ({},1), got blank",
        width - 1
    );
}

#[test]
fn graph_ribbon_node_row_is_horizontally_centered_in_pane() {
    // On a wide terminal, the graph ribbon's first stage glyph should
    // sit roughly equidistant from the pane's left/right edges —
    // satisfying AC-1's "content centered within each pane".
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
    let app = App::load(root).expect("workflow should load");
    let width: u16 = 200;
    let height: u16 = 30;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let buffer = terminal.backend().buffer();
    // Find the row containing the first stage name (e.g. "design").
    let first_stage = &app.as_overview().expect("overview").definition().stages[0].name;
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
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

#[test]
fn dashboard_footer_lists_p3_capability_hints_when_preview_closed() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
    let app = App::load(root).expect("workflow should load");
    let mut terminal = Terminal::new(TestBackend::new(180, 24)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(rendered.contains("/: search"));
    assert!(rendered.contains(": command"));
    assert!(!rendered.contains(":: command"));
    assert!(rendered.contains("T/M/A/R: views"));
    let hints = crate::ui::footer::status_footer_hints(app.as_session().unwrap());
    assert!(hints.iter().any(|(label, _)| label == ": command"));
    assert!(!hints.iter().any(|(label, _)| label == ":: command"));
}

#[test]
fn multi_footer_shows_switch_workflow_when_preview_closed() {
    let session = synthetic_session(2);
    let app = App::from_session(session);
    let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(rendered.contains("\u{2190}/\u{2192}: switch workflow"));
    assert!(!rendered.contains("\u{2190}/\u{2192}: preview scroll"));
    assert!(rendered.contains("PgUp/PgDn: page list"));
    assert!(!rendered.contains("PgUp/PgDn: preview scroll"));
}

#[test]
fn multi_footer_shows_preview_scroll_when_preview_open() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let session = synthetic_session(2);
    let mut app = App::from_session(session);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());

    // Preview-open footer shows one consolidated scroll pill (the full key
    // vocabulary, incl. horizontal scroll, lives in the help popup).
    assert!(rendered.contains("scroll: Space/b PgUp/Dn g/G"));
    assert!(!rendered.contains("\u{2190}/\u{2192}: switch workflow"));
    assert!(!rendered.contains("PgUp/PgDn: page list"));
}

// --- AC-2: tab bar workflow switcher (multi-workflow only) ---
fn synthetic_session(n: usize) -> crate::app::OverviewSession {
    use crate::app::{OverviewSession, OverviewState};
    use spacetop_core::discovery::DiscoveredWorkflow;
    use spacetop_core::domain::{StageDefinition, WorkflowDefinition, WorkflowSnapshot};
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
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items: Vec::new(),
        parse_errors: Vec::new(),
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
        rendered.contains("Workflow0 | Workflow1 | Workflow2"),
        "tab strip must show ratatui tabs, got render snippet:\n{rendered}"
    );
    for i in 0..3 {
        assert!(
            rendered.contains(&format!("Workflow{i}")),
            "tab bar missing workflow tab #{i}"
        );
    }
}

#[test]
fn multi_session_renders_dashboard_inside_workflow_tabs_panel() {
    let session = synthetic_session(2);
    let mut app = App::from_session(session);
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let buffer = terminal.backend().buffer();

    let workflow_graph_y = find_text(buffer, "plan")
        .into_iter()
        .map(|(_, y)| y)
        .filter(|y| *y > 0)
        .min()
        .expect("workflow graph title should render inside tab panel");
    let tasks_y = find_text(buffer, "Tasks")[0].1;
    let preview_y = find_text(buffer, "Preview")[0].1;

    assert!(
        workflow_graph_y <= 3,
        "workflow graph should start inside the workflow tab panel, not below a separate tab strip"
    );
    assert!(
        tasks_y > workflow_graph_y && preview_y > workflow_graph_y,
        "task list and preview should render inside the selected workflow tab panel"
    );
}

#[test]
fn multi_session_tabs_are_borderless_and_do_not_dim_dashboard_content() {
    let session = synthetic_session(2);
    let app = App::from_session(session);
    let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let buffer = terminal.backend().buffer();

    assert_eq!(
        buffer[(0, 0)].symbol(),
        " ",
        "workflow tabs should not draw an outer border"
    );
    let plan_pos = find_text(buffer, "plan")
        .into_iter()
        .find(|(_, y)| *y > 1)
        .expect("workflow graph plan label should render");
    assert!(
        !buffer[plan_pos]
            .style()
            .add_modifier
            .contains(Modifier::DIM),
        "dashboard content inside workflow tabs should not inherit dim tab styling"
    );
}

#[test]
fn single_session_omits_tab_bar() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
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
    // Collect oklch-derived Rgb colors for this workflow's stages.
    let stage_colors: std::collections::HashSet<Color> = app
        .snapshot()
        .definition
        .stage_colors
        .values()
        .copied()
        .map(crate::ui::color::to_color)
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
    let app = app_with_items(vec![item("001", "Synthetic active task", "Body")]);
    let selected = app.selected_item().expect("selected").clone();
    let expected = super::stage_color(&selected.status);
    let status_value = selected.status.clone();
    let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let buffer = terminal.backend().buffer();
    let label_chars: [&str; 10] = ["s", "t", "a", "t", "u", "s", ":", " ", "\u{25CF}", " "];
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
        rendered.contains("switch to next workflow"),
        "help popup must list workflow switching in multi when preview is closed"
    );
    assert!(
        rendered.contains("pick workflow"),
        "multi help should mention pick workflow"
    );

    let session = synthetic_session(2);
    let mut app = App::from_session(session);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        rendered.contains("scroll preview right"),
        "help popup must list preview scrolling when preview is open"
    );
    assert!(
        rendered.contains("Space / PgDn   page preview down"),
        "preview-open help should describe Space/PgDn as page-down preview scroll"
    );
    assert!(
        rendered.contains("g / G          preview top / bottom"),
        "preview-open help should list g/G as top/bottom jumps"
    );
    assert!(
        !rendered.contains("switch to next workflow"),
        "preview-open help should not show workflow switching on arrows"
    );
    assert!(
        !rendered.contains("entity timeline (preview closed)"),
        "preview-open help should not list preview-closed capability views"
    );

    // Single session: the existing `App::load` path produces a pinned
    // single session whose help omits cycle hints.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
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
