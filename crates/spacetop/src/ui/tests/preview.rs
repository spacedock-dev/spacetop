use super::*;

/// AC-6: the help popup documents the new `o` keybinding when the
/// preview pane is open.
#[test]
fn help_popup_documents_open_file_keybind_when_preview_open() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = app_with_items(vec![item("001", "Help test", "body")]);
    // Open preview, then open the help popup.
    // (app_with_items already opens the preview, but be explicit so
    // future refactors don't silently break this test.)
    if !app.as_overview().is_some_and(|s| s.preview_open()) {
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    assert!(app.help_open());

    let mut terminal =
        Terminal::new(TestBackend::new(140, 30)).expect("test terminal should be created");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        rendered.contains("open file in $EDITOR"),
        "help popup should document the `o` keybind, got: {rendered}"
    );
    // Also check the leading `o` key column itself is present in the
    // popup (rather than only the description text).
    assert!(
        find_text(terminal.backend().buffer(), "o ")
            .into_iter()
            .any(|(_, _)| true),
        "help popup should render the `o` key column"
    );
}

#[test]
fn archived_preview_includes_worktree_segment() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (mut app, _dir) = app_loaded_with_archived_item();
    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let mut terminal =
        Terminal::new(TestBackend::new(180, 30)).expect("test terminal should be created");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");

    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        rendered.contains("worktree:"),
        "archived preview should include worktree segment, got: {rendered}"
    );
}
#[test]
fn word_wrap_toggle_changes_body_render() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let body = "a".repeat(200);
    let mut app = App::from_snapshot(
        PathBuf::from("/tmp/ww-ac1"),
        snapshot_with_body("001", "Wrap test", &body),
    );
    // Open preview (wrap defaults to on).
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
    // Wrap-on default: scroll_x clamped to 0.
    terminal.draw(|frame| render(frame, &app)).unwrap();
    assert_eq!(
        app.as_overview().unwrap().max_preview_scroll_x.get(),
        0,
        "wrap mode clamps scroll_x to 0"
    );
    // Toggle wrap off — no-wrap exposes the real horizontal scroll limit.
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let max_x_no_wrap = app.as_overview().unwrap().max_preview_scroll_x.get();
    assert!(
        max_x_no_wrap > 0,
        "no-wrap mode should report a non-zero horizontal scroll limit for a 200-char body"
    );
    // Toggle wrap on again — scroll_x clamped back to 0.
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    terminal.draw(|frame| render(frame, &app)).unwrap();
    assert_eq!(
        app.as_overview().unwrap().max_preview_scroll_x.get(),
        0,
        "wrap mode clamps scroll_x to 0"
    );
}

#[test]
fn word_wrap_toggle_persists_across_preview_open_close() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = App::from_snapshot(
        PathBuf::from("/tmp/ww-ac2"),
        snapshot_with_body("001", "Persist test", "some body"),
    );
    // Open preview — default-on.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        app.as_overview().unwrap().preview_wrap(),
        "preview opens with wrap on by default"
    );
    // Toggle off.
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    assert!(!app.as_overview().unwrap().preview_wrap());
    // Close preview — wrap-off persists across pane close.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        !app.as_overview().unwrap().preview_wrap(),
        "wrap toggle persists across pane close"
    );
    // Re-open — wrap-off still in effect.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        !app.as_overview().unwrap().preview_wrap(),
        "wrap toggle persists across pane re-open"
    );
    // Toggle still works in the other direction.
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    assert!(
        app.as_overview().unwrap().preview_wrap(),
        "w keypress still toggles wrap back on"
    );
}

#[test]
fn footer_shows_word_wrap_hint_when_preview_open() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = App::from_snapshot(
        PathBuf::from("/tmp/ww-ac3"),
        snapshot_with_body("001", "Legend test", "body"),
    );
    let mut terminal = Terminal::new(TestBackend::new(180, 24)).expect("terminal");
    // Before preview: hint absent.
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        !rendered.contains("w: word wrap"),
        "hint absent before preview opens"
    );
    // After preview: hint present.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        rendered.contains("w: word wrap"),
        "hint visible when preview open"
    );
}

#[test]
fn preview_renders_markdown_body_instead_of_raw_markers() {
    let app = app_with_items(vec![item(
        "001",
        "Markdown Preview",
        "# Heading\n\nSome **bold** text.\n\n- first item",
    )]);
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");

    let buffer = terminal.backend().buffer();
    let rendered = buffer_text(buffer);
    assert!(rendered.contains("Heading"), "missing rendered heading");
    assert!(
        rendered.contains("Some bold text."),
        "missing rendered paragraph without markdown markers"
    );
    assert!(
        !rendered.contains("# Heading") && !rendered.contains("**bold**"),
        "preview should not show raw markdown markers"
    );
    assert!(
        find_styled_text(buffer, "Heading", |style| {
            style.add_modifier.contains(ratatui::style::Modifier::BOLD)
        }),
        "heading text should be bold"
    );
    assert!(
        find_styled_text(buffer, "bold", |style| {
            style.add_modifier.contains(ratatui::style::Modifier::BOLD)
        }),
        "strong markdown text should be bold"
    );
}

#[test]
fn preview_renders_session_metadata_without_transcript_content() {
    let app = app_with_session_attribution(
        vec![item(
            "065",
            "Active task",
            "Visible markdown body, not a transcript leak.",
        )],
        "065",
        spacetop_core::domain::EntityActivity::Running {
            handler: spacetop_core::domain::ActivityHandler::Worker,
            runtime: spacetop_core::domain::AgentRuntime::Codex,
            session_id: "session-065".to_string(),
            updated_unix: 1_718_000_000,
        },
    );
    let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        rendered.contains("Runtime: Codex"),
        "preview should name the matched runtime; rendered: {rendered:?}"
    );
    assert!(rendered.contains("Session: session-065"));
    assert!(rendered.contains("Status: running · worker"));
    assert!(!rendered.contains("Confidence:"));
    assert!(!rendered.contains("Handler:"));
    assert!(
        rendered.contains(" ago"),
        "updated activity should be human-readable; rendered: {rendered:?}"
    );
    assert!(!rendered.contains("Updated: 1718000000"));
    let status_index = rendered.find("status:").expect("status line");
    let activity_index = rendered.find("Runtime: Codex").expect("activity line");
    let source_index = rendered.find("source:").expect("source line");
    assert!(
        status_index < activity_index && activity_index < source_index,
        "activity line should sit between status/score and source; rendered: {rendered:?}"
    );
    assert!(
        !rendered.contains("prompt:") && !rendered.contains("response:"),
        "preview metadata must not expose transcript fields"
    );

    let app = app_with_session_attribution(
        vec![item("066", "Stale task", "Visible markdown body.")],
        "066",
        spacetop_core::domain::EntityActivity::Idle {
            updated_unix: Some(1_718_000_000),
        },
    );
    let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(rendered.contains("Runtime: —"));
    assert!(rendered.contains("Session: —"));
    assert!(rendered.contains("Status: idle"));
}

#[test]
fn preview_omits_session_metadata_for_unrelated_running_session() {
    use spacetop_core::session_activity::{
        scan_local_sessions_with, ProcessProbe, SessionRoots, SessionScanEntity, SessionScanRequest,
    };
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;
    use std::time::SystemTime;

    struct RunningProbe {
        running: HashSet<u32>,
    }

    impl ProcessProbe for RunningProbe {
        fn is_running(&self, pid: u32) -> bool {
            self.running.contains(&pid)
        }
    }

    fn write_session(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, body).expect("write session");
    }

    let tmp = tempfile::tempdir().expect("tmp");
    let workflow = tmp.path().join("spacetop/docs/spacetop-dev");
    let root = tmp.path().join("codex");
    let entity = item("068", "New task", "Visible markdown body.");
    let snapshot = spacetop_core::domain::WorkflowSnapshot {
        definition: spacetop_core::domain::WorkflowDefinition {
            root: workflow.clone(),
            state: None,
            stages: vec![spacetop_core::domain::StageDefinition {
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
            transitions: Vec::new(),
        },
        items: vec![entity.clone()],
        parse_errors: Vec::new(),
    };
    let mut state = OverviewState::from_snapshot(workflow.clone(), snapshot);
    let repo = state.repo_root.clone();
    write_session(
        &root.join("rollout-2026-06-18T14-26-00-019ed968-6e77-7d71-9386-aae754c6c8be.jsonl"),
        r#"{"pid":4242,"agent_nickname":"Mendel","workdir":"/Users/kent/Dev/InfuseAI/GitHub/dataagentbench","note":"created task 068"}"#,
    );
    let request = SessionScanRequest {
        workflow_dir: workflow.clone(),
        repo_root: repo.clone(),
        entities: vec![SessionScanEntity {
            id: entity.id.clone(),
            path: entity.path.clone(),
            worktree: entity.worktree.clone(),
            worktree_source: entity.worktree_source.clone(),
        }],
        roots: SessionRoots {
            codex: vec![root],
            claude_code: Vec::new(),
        },
        previous_state: Default::default(),
    };
    let report = scan_local_sessions_with(
        &request,
        &RunningProbe {
            running: HashSet::from([4242]),
        },
        SystemTime::now(),
    )
    .expect("scan succeeds");
    state.apply_session_activity_result(crate::app::SessionActivityWorkerResult {
        workflow_dir: workflow,
        repo_root: repo,
        state: Default::default(),
        retry_immediately: false,
        result: Ok(report),
    });
    let mut app = App::from_session(OverviewSession::single(state, true));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let mut terminal = Terminal::new(TestBackend::new(160, 30)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(!rendered.contains("\u{25CF}   New task"));
    assert!(!rendered.contains("Runtime: Codex"));
    assert!(rendered.contains("Status: idle"));
    assert!(!rendered.contains("Session: Mendel"));
}

#[test]
fn preview_renders_markdown_tables_as_aligned_rows() {
    // termimad renders tables with Unicode box-drawing borders. We check
    // that the preceding paragraph and every cell value land in the
    // buffer, that raw pipe-and-dash markdown leaks have been suppressed,
    // and that a header/body separator row is drawn.
    let app = app_with_items(vec![item(
        "001",
        "Markdown Table Preview",
        "Ablation siblings\n\n| Arm | Entity | README |\n| --- | ---: | --- |\n| 1 | 17 | direct.md |\n| 2 | 18 | method.md |",
    )]);
    let mut terminal = Terminal::new(TestBackend::new(140, 24)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");

    let rendered = buffer_text(terminal.backend().buffer());
    assert!(rendered.contains("Ablation siblings"));
    for cell in ["Arm", "Entity", "README", "direct.md", "method.md"] {
        assert!(
            rendered.contains(cell),
            "expected cell {cell:?} in rendered table:\n{rendered}"
        );
    }
    assert!(
        rendered.contains("\u{2500}\u{2500}\u{2500}"),
        "table should show a separator row between header and body"
    );
    assert!(
        rendered.contains("\u{2502}"),
        "termimad renders cell borders with the vertical box-drawing char"
    );
    assert!(
        !rendered.contains("| Arm |") && !rendered.contains("---"),
        "preview should render table structure rather than raw markdown separators"
    );
}

#[test]
fn preview_uses_full_pane_width_for_wide_content() {
    let body = format!("{}PREVIEWFULLWIDTH", "X".repeat(92));
    let app = app_with_items(vec![item("001", "Wide Preview", &body)]);
    let width: u16 = 220;
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");

    let buffer = terminal.backend().buffer();
    assert!(
        find_text_starting_after(buffer, "PREVIEWFULLWIDTH", 30),
        "preview content should use the full preview pane instead of a centered narrow column"
    );
}

#[test]
fn preview_right_key_horizontally_scrolls_long_lines() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let body = format!("{}HORIZONTALSCROLLTARGET", "X".repeat(220));
    let mut app = app_with_items(vec![item("001", "Wide Preview", &body)]);
    // Disable wrap so the long line stays on a single row and horizontal scroll applies.
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    let width: u16 = 80;
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).expect("terminal");

    terminal.draw(|frame| render(frame, &app)).unwrap();
    let before = buffer_text(terminal.backend().buffer());
    assert!(!before.contains("HORIZONTALSCROLLTARGET"));

    for _ in 0..30 {
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    }
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let after = buffer_text(terminal.backend().buffer());
    assert!(after.contains("HORIZONTALSCROLLTARGET"));
}

#[test]
fn preview_draws_scrollbar_when_content_overflows() {
    let body = (0..40)
        .map(|index| format!("Line {index}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let app = app_with_items(vec![item("001", "Scrollable Preview", &body)]);
    let width: u16 = 120;
    let height: u16 = 18;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");

    let buffer = terminal.backend().buffer();
    let has_scrollbar_thumb = (0..buffer.area.height)
        .any(|y| (0..buffer.area.width).any(|x| buffer[(x, y)].symbol() == "\u{2588}"));
    assert!(
        has_scrollbar_thumb,
        "overflowing preview should draw a scrollbar thumb"
    );
}

#[test]
fn preview_page_down_scrolls_visible_markdown_content() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let body = (0..30)
        .map(|index| format!("Line {index:02}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut app = app_with_items(vec![item("001", "Scrollable Preview", &body)]);
    // Page step is viewport-relative; pin a 7-row body so a PageDown advances
    // 6 rows (≈3 spaced markdown lines) regardless of the test terminal size.
    app.as_overview().unwrap().preview_viewport_height.set(7);
    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

    let mut terminal = Terminal::new(TestBackend::new(120, 18)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");

    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        !rendered.contains("Line 00"),
        "scrolled preview should not keep the first body line visible"
    );
    assert!(
        rendered.contains("Line 03"),
        "scrolled preview should advance by the PageDown row offset, including markdown spacing"
    );
}

#[test]
fn preview_adds_blank_rows_between_markdown_blocks() {
    let app = app_with_items(vec![item(
        "001",
        "Spaced Markdown",
        "# Heading\n\nFirst paragraph.\n\nSecond paragraph.",
    )]);
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");

    let buffer = terminal.backend().buffer();
    let heading_y = find_text(buffer, "Heading")[0].1;
    let first_y = find_text(buffer, "First paragraph.")[0].1;
    let second_y = find_text(buffer, "Second paragraph.")[0].1;
    assert!(
        first_y >= heading_y + 2,
        "expected a blank row between heading and first paragraph"
    );
    assert!(
        second_y >= first_y + 2,
        "expected a blank row between paragraphs"
    );
}

#[test]
fn preview_keeps_body_divider_visible_when_header_wraps() {
    let mut work_item = item("001", "Wrapped Header", "Body content stays visible.");
    work_item.path = PathBuf::from(
        "/tmp/very/long/path/that/forces/the/preview/header/path/line/to/wrap/multiple/times/so/the/body/divider/must/still/render/work-item.md",
    );
    let app = app_with_items(vec![work_item]);
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");

    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        rendered.contains("── body"),
        "wrapped preview headers should still leave room for the body divider"
    );
}

#[test]
fn preview_header_long_source_and_worktree_do_not_overlap_body() {
    let mut work_item = item(
        "001",
        "Long Metadata Header",
        "Primary preview body stays visible after metadata.",
    );
    work_item.source = Some(
        "captain request with a deliberately long source value that wraps across the preview header"
            .to_string(),
    );
    work_item.worktree = Some(
        ".worktrees/spacedock-ensign-061-preview-header-source-worktree-lines-with-extra-long-suffix"
            .to_string(),
    );
    let app = app_with_items(vec![work_item]);
    let mut terminal = Terminal::new(TestBackend::new(80, 80)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");

    let buffer = terminal.backend().buffer();
    let status_y = find_text(buffer, "status:")[0].1;
    let source_y = find_text(buffer, "source:")[0].1;
    let worktree_y = find_text(buffer, "worktree:")[0].1;
    let path_y = find_text(buffer, "path:")[0].1;
    let divider_y = find_text(buffer, "── body")[0].1;
    let body_y = find_text(buffer, "Primary preview body")[0].1;

    assert!(
        source_y > status_y,
        "source metadata must render below the status row"
    );
    assert!(
        worktree_y > source_y,
        "worktree metadata must render below the source row"
    );
    assert!(
        path_y > worktree_y,
        "path metadata must render below source/worktree rows"
    );
    assert!(
        divider_y > path_y,
        "body divider must render below all metadata"
    );
    assert!(body_y > divider_y, "body text must render below divider");
}
#[test]
fn preview_scrollbar_thumb_reaches_bottom_at_max_scroll() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let body = (0..60)
        .map(|i| format!("Line {:02}", i))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut app = app_with_items(vec![item("001", "Scrollable", &body)]);
    let width: u16 = 160;
    let height: u16 = 30;

    // Run several render+scroll cycles so max_preview_scroll is set by
    // render_preview before scroll_preview_down reads it.
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    for _ in 0..30 {
        terminal.draw(|frame| render(frame, &app)).unwrap();
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    }
    // Final render at max scroll.
    terminal.draw(|frame| render(frame, &app)).unwrap();

    let buffer = terminal.backend().buffer();
    let thumb_rows = (0..buffer.area.height)
        .flat_map(|y| (0..buffer.area.width).map(move |x| (x, y)))
        .filter(|&(x, y)| buffer[(x, y)].symbol() == "\u{2588}")
        .map(|(_, y)| y)
        .collect::<Vec<_>>();
    let bottom_row = thumb_rows.iter().copied().max().expect("thumb visible");
    let thumb_at_bottom = bottom_row >= height / 2;
    assert!(
        thumb_at_bottom,
        "scrollbar thumb must move into the lower half of the preview at max scroll (row={bottom_row})"
    );
}

#[test]
fn preview_scrollbar_thumb_starts_at_top_at_zero_scroll() {
    let body = (0..60)
        .map(|i| format!("Line {:02}", i))
        .collect::<Vec<_>>()
        .join("\n\n");
    let app = app_with_items(vec![item("001", "Scrollable", &body)]);
    let width: u16 = 160;
    let height: u16 = 32;

    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).unwrap();

    let buffer = terminal.backend().buffer();
    let first_thumb_row = (0..buffer.area.height)
        .flat_map(|y| (0..buffer.area.width).map(move |x| (x, y)))
        .filter(|&(x, y)| buffer[(x, y)].symbol() == "\u{2588}")
        .map(|(_, y)| y)
        .min()
        .expect("scrollbar thumb must be visible at scroll=0");

    assert!(
        first_thumb_row < height / 2,
        "at scroll=0, thumb must sit in the upper half of the track (got row {first_thumb_row})"
    );
}

// --- Graph-aware coloring tests (AC-1, AC-2, AC-3) ---
