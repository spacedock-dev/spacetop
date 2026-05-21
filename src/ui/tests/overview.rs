use super::*;

#[test]
fn renders_real_workflow_summary_task_list_and_preview() {
    let app = app_with_items(vec![item(
        "001",
        "Synthetic active task",
        "This body gives the preview pane stable content.",
    )]);
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
    assert!(rendered.contains(&format!("status: ● {}", selected.status)));
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
fn overview_hides_preview_until_enter_opens_preview_mode() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let root = PathBuf::from("/tmp/spacetop-hidden-preview");
    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: root.clone(),
            stages: vec![StageDefinition {
                name: "design".to_string(),
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
        },
        items: vec![item("001", "Hidden Preview", "Body")],
        parse_errors: Vec::new(),
    };
    let mut app = App::from_snapshot(root, snapshot);

    let mut terminal = Terminal::new(TestBackend::new(140, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(rendered.contains("Tasks"));
    assert!(!rendered.contains("Preview  ·"));

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(rendered.contains("Preview  ·"));
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
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

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

#[test]
fn preview_opens_on_right_in_wide_terminals_and_bottom_in_taller_ones() {
    let app = app_with_items(vec![item("001", "Placement", "Body")]);

    let mut wide = Terminal::new(TestBackend::new(180, 24)).expect("wide terminal");
    wide.draw(|frame| render(frame, &app)).unwrap();
    let wide_buffer = wide.backend().buffer();
    let tasks_wide = find_text(wide_buffer, "Tasks")[0];
    let preview_wide = find_text(wide_buffer, "Preview")[0];
    assert_eq!(tasks_wide.1, preview_wide.1);
    assert!(preview_wide.0 > tasks_wide.0);

    let mut tall = Terminal::new(TestBackend::new(80, 180)).expect("tall terminal");
    tall.draw(|frame| render(frame, &app)).unwrap();
    let tall_buffer = tall.backend().buffer();
    let tasks_tall = find_text(tall_buffer, "Tasks")[0];
    let preview_tall = find_text(tall_buffer, "Preview")[0];
    assert!(preview_tall.1 > tasks_tall.1);
}

#[test]
fn bottom_preview_compacts_metadata_into_one_line() {
    let mut work_item = item("001", "Bottom Preview", "Body");
    work_item.score = Some(0.75);
    work_item.source = Some("captain".to_string());
    let app = app_with_items(vec![work_item]);

    let mut terminal = Terminal::new(TestBackend::new(80, 180)).expect("tall terminal");
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(rendered.contains("status: ● design"));
    assert!(rendered.contains("score: 0.75"));
    assert!(rendered.contains("source: captain"));
}

#[test]
fn bottom_preview_shows_worktree_when_set() {
    let mut work_item = item("001", "WT", "Body");
    work_item.worktree = Some(".worktrees/ensign-foo".to_string());
    let app = app_with_items(vec![work_item]);

    let mut terminal = Terminal::new(TestBackend::new(80, 180)).expect("tall terminal");
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        rendered.contains("worktree: ensign-foo"),
        "expected worktree basename in bottom preview, got: {rendered}"
    );
    assert!(
        !rendered.contains(".worktrees/ensign-foo"),
        "bottom preview must render basename only, not full path"
    );
}

#[test]
fn left_preview_shows_worktree_when_set() {
    let mut work_item = item("001", "WT", "Body");
    work_item.worktree = Some(".worktrees/ensign-foo".to_string());
    let app = app_with_items(vec![work_item]);

    let mut terminal = Terminal::new(TestBackend::new(180, 24)).expect("wide terminal");
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        rendered.contains("worktree: ensign-foo"),
        "expected worktree basename in left preview, got: {rendered}"
    );
    assert!(
        !rendered.contains(".worktrees/ensign-foo"),
        "left preview must render basename only, not full path"
    );
}

#[test]
fn preview_renders_em_dash_for_empty_worktree() {
    let work_item = item("001", "WT", "Body");
    let app = app_with_items(vec![work_item]);

    let mut terminal = Terminal::new(TestBackend::new(80, 180)).expect("tall terminal");
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        rendered.contains("worktree: \u{2014}"),
        "expected em-dash empty marker, got: {rendered}"
    );
    assert!(
        rendered.contains("status: ● design"),
        "surrounding header should remain intact"
    );
}
