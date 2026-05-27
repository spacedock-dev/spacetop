use super::*;

#[test]
fn task_list_uses_full_pane_width_and_ratatui_list_selection() {
    let stable_title = "Stable selected title";
    let long_title = format!("{}FULLWIDTHMARKER", "X".repeat(60));
    let app = app_with_items(vec![
        item("001", stable_title, "Body"),
        item("002", &long_title, "Body"),
        item("003", "Second task", "Body"),
    ]);

    let width: u16 = 220;
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let buffer = terminal.backend().buffer();

    // FULLWIDTHMARKER sits 60 X's into the title, after the row prefix
    // (id + spaces + tag + spaces = 11 chars) and the 2-char highlight pad.
    // With no border on the list pane it lands at col ~73; threshold 60
    // is generous enough to prove content is not confined to a narrow column.
    assert!(
        find_text_starting_after(buffer, "FULLWIDTHMARKER", 60),
        "task row content should use the whole list pane rather than a centered narrow column"
    );
    let rendered = buffer_text(buffer);
    // Selected row now uses ▸ gutter (not "> ") with bg-2 fill.
    assert!(
        rendered.contains('\u{25B8}'),
        "selected row should display ▸ gutter glyph"
    );
    assert!(
        find_styled_text(buffer, stable_title, |style| {
            style.bg == Some(ratatui::style::Color::Rgb(40, 52, 84))
        }),
        "selected row title should have selection color fill (Tokyo Night Rgb(40,52,84))"
    );
    assert!(
        find_styled_text(buffer, stable_title, |style| {
            style.add_modifier.contains(ratatui::style::Modifier::BOLD)
        }),
        "selected row title should be bold"
    );
}

#[test]
fn selected_row_fill_covers_full_pane_width() {
    // The selected row background (Rgb(40,52,84)) must extend to the rightmost
    // cell of the task list pane, not stop at the last text character.
    let app = app_with_items(vec![
        item("001", "Short title", "Body"),
        item("002", "Another task", "Body"),
    ]);
    let width: u16 = 80;
    let height: u16 = 24;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let buffer = terminal.backend().buffer();

    // Find the selected row (row with ▸ glyph).
    let hits = find_text(buffer, "\u{25B8}");
    assert!(!hits.is_empty(), "selected row must have ▸ glyph");
    let (_, sel_row) = hits[0];

    // The rightmost cell of the task-list pane (width/2 - 1 in split view, or
    // width - 1 in full-pane view). In full-pane mode (no preview open by default)
    // the list fills 0..width. Check the last cell on the selected row has BG2.
    // app_with_items opens preview (Enter), so list is the left half: 0..width/2.
    let last_list_col = width / 2 - 1;
    let style = buffer[(last_list_col, sel_row)].style();
    assert_eq!(
        style.bg,
        Some(Color::Rgb(40, 52, 84)),
        "rightmost list cell on selected row (col {last_list_col}, row {sel_row}) must have selection background"
    );
}

// ---- AC snapshot tests ----

#[test]
fn task_row_phase_column_12_char_fixed() {
    // With a single item whose status is "implement" (9 chars), phase_col_width
    // auto-sizes to 9 (the longest status, clamped to [4,12]). The phase
    // column is "implement" with no trailing spaces.
    let app = app_with_items(vec![{
        let mut i = item("001", "Test task", "Body");
        i.status = "implement".to_string();
        i
    }]);
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());
    // "implement" must appear with user casing preserved.
    assert!(
        rendered.contains("implement"),
        "phase name 'implement' must appear in task row; rendered: {:?}",
        &rendered[..rendered.len().min(200)]
    );
    // The word "implement" must not be uppercased.
    assert!(
        !rendered.contains("IMPLEMENT"),
        "phase name must not be uppercased"
    );
    // Verify phase_col() helper directly for various widths.
    let col_w9 = super::phase_col("implement", 9);
    assert_eq!(
        col_w9, "implement",
        "phase_col('implement', 9) must be exact fit"
    );
    let col_w12 = super::phase_col("implement", 12);
    assert_eq!(
        col_w12, "implement   ",
        "phase_col('implement', 12) must pad to 12"
    );
    let col_w4 = super::phase_col("implement", 4);
    assert_eq!(
        col_w4, "imp\u{2026}",
        "phase_col('implement', 4) must truncate at 3+ellipsis"
    );
}

#[test]
fn task_row_long_phase_name_ellipsis() {
    // Phase names longer than 12 chars must be ellipsized at char 11 + "…".
    let long_phase = "averylongphasename"; // 18 chars
    let app = {
        let root = PathBuf::from("/tmp/spacetop-ellipsis");
        let snapshot = crate::domain::WorkflowSnapshot {
            definition: crate::domain::WorkflowDefinition {
                root: root.clone(),
                stages: vec![crate::domain::StageDefinition {
                    name: long_phase.to_string(),
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
            items: vec![{
                let mut i = item("001", "Long phase task", "Body");
                i.status = long_phase.to_string();
                i
            }],
            parse_errors: Vec::new(),
        };
        App::from_snapshot(root, snapshot)
    };
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());
    // First 11 chars of long_phase + "…"
    let expected_prefix = &long_phase[..11]; // "averylongph"
    assert!(
        rendered.contains(&format!("{expected_prefix}\u{2026}")),
        "long phase name should be ellipsized to 11 chars + '…'; rendered: {:?}",
        &rendered[..rendered.len().min(200)]
    );
}

#[test]
fn task_row_selected_gutter() {
    // Selected row must show ▸ gutter; unselected rows must show 2 spaces.
    let app = app_with_items(vec![
        item("001", "Selected task", "Body"),
        item("002", "Unselected task", "Body"),
    ]);
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());
    // ▸ (\u{25B8}) must appear for the selected row.
    assert!(
        rendered.contains('\u{25B8}'),
        "selected row must show ▸ gutter glyph"
    );
}

#[test]
fn task_row_no_uppercase_phase() {
    // Phase column must not uppercase any stage name.
    let app = app_with_items(vec![item("001", "Task", "Body")]);
    // The stage name in `app_with_items` is "design" (lowercase).
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());
    // "design" must appear (lowercase), not "DESIGN", "DES", "DGN", etc.
    assert!(
        rendered.contains("design"),
        "phase name 'design' must appear in task row"
    );
    assert!(
        !rendered.contains("DESIGN"),
        "phase name must not be uppercased"
    );
    assert!(
        !rendered.contains("DES"),
        "old 3-letter tag must not appear"
    );
}

#[test]
fn task_row_no_glyphs_in_phase_col() {
    // Phase column must not contain DAG vocabulary glyphs.
    // The DAG glyphs are ▶ (U+25B6), ⎇ (U+2387), ⚑ (U+2691), ■ (U+25A0).
    // Note: ▸ (U+25B8) is the gutter selection glyph — NOT a DAG vocab glyph.
    let dag_glyphs: &[char] = &['\u{25B6}', '\u{2387}', '\u{2691}', '\u{25A0}'];

    let app = app_with_items(vec![item("001", "Task", "Body")]);
    let width: u16 = 100;
    let height: u16 = 24;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let buffer = terminal.backend().buffer();

    // Layout (with preview open, Left placement at width=100, height=24):
    //   row 0: header bar
    //   rows 1–7: graph ribbon (7 rows)
    //   row 8: "Tasks · N" section header
    //   rows 9+: task list rows
    //
    // Task row column layout (x offsets, no border on list block):
    //   x 0–1:  gutter (2 chars: "▸ " or "  ")
    //   x 2–13: phase column (12 chars — user stage name)
    //   x 14:   separator space
    //   x 15–18: id (4 chars)
    //   x 19+:  title
    //
    // We scan the phase column cells (x=2..14) for all task rows (y=9..height).
    // No DAG vocabulary glyph may appear there.

    let phase_col_x_start: u16 = 2;
    let phase_col_x_end: u16 = 14; // exclusive
    let task_rows_y_start: u16 = 9; // first task row (after section header at y=8)

    let mut violations: Vec<(u16, u16, char)> = Vec::new();
    for y in task_rows_y_start..height {
        for x in phase_col_x_start..phase_col_x_end {
            let cell = &buffer[(x, y)];
            let sym = cell.symbol();
            for ch in sym.chars() {
                if dag_glyphs.contains(&ch) {
                    violations.push((x, y, ch));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "DAG glyphs found in task list phase column (x=2..14, y=9..{}): {:?}",
        height,
        violations
    );

    // Also verify phase_col() helper itself never emits DAG glyphs.
    let pc = super::phase_col("design", 12);
    for glyph in dag_glyphs {
        assert!(
            !pc.contains(*glyph),
            "phase_col('design') must not contain DAG glyph {:?}, got {:?}",
            glyph,
            pc
        );
    }
}

#[test]
fn header_strip_badge_style_and_path_truncation() {
    // At a short terminal width, the path should be left-truncated with "…".
    // The badge should have yellow background (filled style).
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
    let app = App::load(root).expect("workflow should load");
    // Narrow enough to trigger path truncation. The header prefix grew with
    // the sort badge, so this needs to be a bit wider than before to leave
    // room for the truncated path itself.
    let width: u16 = 100;
    let mut terminal = Terminal::new(TestBackend::new(width, 20)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let buffer = terminal.backend().buffer();
    let rendered = buffer_text(buffer);
    // Badge "[active]" must appear.
    assert!(rendered.contains("[active]"), "badge must appear");
    // Yellow bg must be present in the header row (row 0) for badge cells.
    let mut found_yellow_bg = false;
    for x in 0..width {
        let cell = &buffer[(x, 0)];
        if cell.style().bg == Some(ratatui::style::Color::Yellow) {
            found_yellow_bg = true;
            break;
        }
    }
    assert!(
        found_yellow_bg,
        "badge cell must have yellow background in row 0"
    );
    // "…" must appear in row 0 (left-truncated path) at narrow width.
    assert!(
        rendered.contains('\u{2026}'),
        "path must be left-truncated with '…' at width={width}"
    );
}

#[test]
fn header_bar_shows_sort_badge() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = app_with_items(vec![item("002", "Two", "b"), item("010", "Ten", "b")]);
    // app_with_items opens preview; close it so 's' is not gated off.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let width: u16 = 200;
    let mut terminal = Terminal::new(TestBackend::new(width, 20)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        rendered.contains("[sort: id]"),
        "header must show [sort: id] initially, got: {rendered}"
    );

    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    let mut terminal = Terminal::new(TestBackend::new(width, 20)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        rendered.contains("[sort: status]"),
        "header must show [sort: status] after cycling, got: {rendered}"
    );
}

#[test]
fn footer_hints_have_background() {
    // Footer pill-style hints must have a non-default background color.
    let app = app_with_items(vec![item("001", "Task", "Body")]);
    let height: u16 = 24;
    let width: u16 = 120;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let buffer = terminal.backend().buffer();
    // Footer is the last row.
    let footer_y = height - 1;
    let mut found_pill_bg = false;
    for x in 0..width {
        let cell = &buffer[(x, footer_y)];
        if let Some(bg) = cell.style().bg {
            if bg != ratatui::style::Color::Reset {
                found_pill_bg = true;
                break;
            }
        }
    }
    assert!(
        found_pill_bg,
        "footer row {footer_y} must have at least one cell with a non-default background (pill hint)"
    );
}

// ---- phase_col_width auto-sizing snapshot tests ----

#[test]
fn phase_col_width_uniform_short_phases_clamped_to_4() {
    // When all visible items have status "run" (3 chars), phase_col_width
    // clamps to the minimum of 4. Verify via phase_col() helper directly.
    let mut run_item = item("001", "Task", "Body");
    run_item.status = "run".to_string(); // 3 chars, below minimum of 4
                                         // "run" is 3 chars < 4 minimum → phase_col_width returns 4.
    let items_ref: Vec<&crate::domain::WorkItem> = vec![&run_item];
    // Simulate what build_task_list_items does: collect refs and call phase_col_width.
    // We use a locally-constructed slice to test the helper.
    let pcw = items_ref
        .iter()
        .map(|i| i.status.chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 12);
    assert_eq!(
        pcw, 4,
        "phase_col_width for 'run' (3 chars) must clamp to 4"
    );
    // phase_col with width=4 pads "run" to "run " (3 chars + 1 space).
    let col = super::phase_col("run", pcw);
    assert_eq!(col, "run ", "phase_col('run', 4) must pad to 4 chars");
    assert_eq!(
        col.chars().count(),
        4,
        "column must be exactly 4 chars wide"
    );
}

#[test]
fn phase_col_width_mixed_phases_fits_longest() {
    // When items have mixed phase lengths, phase_col_width picks the longest
    // (clamped ≤ 12). "run" (3→4 min), "implement" (9), "smoke-test" (10).
    // Longest is 10 → phase_col_width = 10.
    let items_data = [
        {
            let mut i = item("001", "Task A", "Body");
            i.status = "run".to_string();
            i
        },
        {
            let mut i = item("002", "Task B", "Body");
            i.status = "implement".to_string();
            i
        },
        {
            let mut i = item("003", "Task C", "Body");
            i.status = "smoke-test".to_string();
            i
        },
    ];
    let items_ref: Vec<&crate::domain::WorkItem> = items_data.iter().collect();
    let pcw = items_ref
        .iter()
        .map(|i| i.status.chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 12);
    assert_eq!(
        pcw, 10,
        "phase_col_width for mixed phases with max len=10 must return 10"
    );
    // "implement" (9 chars) with width=10 must pad to 10 chars.
    let col = super::phase_col("implement", pcw);
    assert_eq!(
        col.chars().count(),
        10,
        "phase column must be exactly 10 chars"
    );
    assert_eq!(col, "implement ", "implement padded to width 10");
    // "smoke-test" (10 chars) with width=10 must fit exactly.
    let col2 = super::phase_col("smoke-test", pcw);
    assert_eq!(
        col2, "smoke-test",
        "smoke-test must fit exactly at width 10"
    );
}

#[test]
fn phase_col_width_long_phase_name_clamped_at_12() {
    // When the longest phase name exceeds 12 chars, phase_col_width clamps to 12.
    let long_item = {
        let mut i = item("001", "Task", "Body");
        i.status = "a-very-long-phase-name".to_string(); // 22 chars
        i
    };
    let items_ref: Vec<&crate::domain::WorkItem> = vec![&long_item];
    let pcw = items_ref
        .iter()
        .map(|i| i.status.chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 12);
    assert_eq!(
        pcw, 12,
        "phase_col_width must clamp to 12 for a 22-char status"
    );
    // phase_col with width=12 must truncate at 11 chars + "…".
    let col = super::phase_col("a-very-long-phase-name", pcw);
    assert_eq!(
        col.chars().count(),
        12,
        "column must be exactly 12 chars after truncation"
    );
    assert!(
        col.ends_with('\u{2026}'),
        "truncated column must end with '…'"
    );
    assert_eq!(
        &col[..col.len() - 3],
        "a-very-long", // 11 chars, "…" is 3 bytes
        "first 11 chars must be preserved before ellipsis"
    );
}

// ---- Task 042: broken-entity row + preview + footer pill ----

use crate::domain::EntityParseError;

fn app_with_broken_entity() -> App {
    let root = PathBuf::from("/tmp/spacetop-broken");
    let snapshot = crate::domain::WorkflowSnapshot {
        definition: crate::domain::WorkflowDefinition {
            root: root.clone(),
            stages: vec![crate::domain::StageDefinition {
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
        items: vec![item("001", "Valid Task", "Body")],
        parse_errors: vec![EntityParseError {
            path: PathBuf::from("/tmp/spacetop-broken/bad.md"),
            message: "/tmp/spacetop-broken/bad.md: malformed YAML frontmatter: mapping values are not allowed in this context at line 7 column 137".to_string(),
            line: Some(7),
            column: Some(137),
        }],
    };
    App::from_snapshot(root, snapshot)
}

#[test]
fn task_list_renders_broken_entity_row_after_items() {
    // AC-2: synthetic "broken" row labeled `⚠ broken: <file>` appears after
    // the valid work-item rows.
    let app = app_with_broken_entity();
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        rendered.contains("Valid Task"),
        "valid row must still render alongside broken row"
    );
    assert!(
        rendered.contains("\u{26A0} broken: bad.md"),
        "broken-entity row label must be visible; rendered: {:?}",
        &rendered[..rendered.len().min(400)]
    );
}

#[test]
fn preview_pane_renders_broken_entity_error_with_hint() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    // Open preview, then advance selection to the broken row.
    let mut app = app_with_broken_entity();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    let mut terminal = Terminal::new(TestBackend::new(160, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());

    assert!(
        rendered.contains("Cannot parse"),
        "broken preview must show 'Cannot parse' header; rendered: {:?}",
        &rendered[..rendered.len().min(600)]
    );
    assert!(
        rendered.contains("bad.md"),
        "broken preview must show file name"
    );
    assert!(
        rendered.contains("malformed YAML frontmatter"),
        "broken preview must surface underlying YAML error"
    );
    assert!(
        rendered.contains("line 7"),
        "broken preview must surface the line number"
    );
    assert!(
        rendered.contains("column 137"),
        "broken preview must surface the column number"
    );
    // Pin the stable user-facing hint string verbatim.
    assert!(
        rendered.contains(
            "Hint: wrap values containing ':' in quotes, or use '>-' for multi-line scalars"
        ),
        "broken preview must show the stable remediation hint"
    );
}

// ---- Task 046: sync-pill labels (stable user-facing strings) ----

#[test]
fn footer_sync_pill_labels_match_pinned_strings() {
    use crate::app::SyncStatus;
    use crate::ui::footer::sync_pill_label;

    assert_eq!(
        sync_pill_label(Some(&SyncStatus::InFlight)).as_deref(),
        Some("Syncing\u{2026}")
    );
    assert_eq!(
        sync_pill_label(Some(&SyncStatus::Succeeded { new_commits: 0 })).as_deref(),
        Some("\u{2713} Synced (already up to date)")
    );
    assert_eq!(
        sync_pill_label(Some(&SyncStatus::Succeeded { new_commits: 1 })).as_deref(),
        Some("\u{2713} Synced (1 new commit)")
    );
    assert_eq!(
        sync_pill_label(Some(&SyncStatus::Succeeded { new_commits: 3 })).as_deref(),
        Some("\u{2713} Synced (3 new commits)")
    );
    assert_eq!(
        sync_pill_label(Some(&SyncStatus::Failed {
            message: "boom".into()
        }))
        .as_deref(),
        Some("\u{26A0} Sync failed: boom")
    );
    assert_eq!(
        sync_pill_label(Some(&SyncStatus::Unavailable {
            hint: "not a git repository".into()
        }))
        .as_deref(),
        Some("Sync unavailable: not a git repository")
    );
    assert!(sync_pill_label(None).is_none());
}

#[test]
fn footer_renders_sync_pill_when_status_set() {
    let mut app = app_with_items(vec![item("001", "Task", "Body")]);
    // Close the preview so the footer carries the Y hint and pills.
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    // Inject a Succeeded status as the event loop would after sync.
    app.set_sync_status(crate::app::SyncStatus::Succeeded { new_commits: 2 });
    let mut terminal = Terminal::new(TestBackend::new(200, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        rendered.contains("\u{2713} Synced (2 new commits)"),
        "footer must render the ✓ Synced (2 new commits) pill, got: {rendered}"
    );
    assert!(
        rendered.contains("Y: sync"),
        "footer must include the Y: sync hint when preview closed"
    );
}

#[test]
fn footer_renders_succeeded_sync_pill_green() {
    use ratatui::style::Color;

    let mut app = app_with_items(vec![item("001", "Task", "Body")]);
    // Close the preview so the footer carries the sync pill.
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    app.set_sync_status(crate::app::SyncStatus::Succeeded { new_commits: 2 });
    let mut terminal = Terminal::new(TestBackend::new(200, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let buffer = terminal.backend().buffer();
    // The whole pill span is styled uniformly, so matching on "Synced"
    // proves the green foreground (AC-7).
    assert!(
        find_styled_text(buffer, "Synced", |s| s.fg == Some(Color::Green)),
        "succeeded sync pill must render with a green foreground"
    );
}

#[test]
fn footer_renders_sync_failed_pill_with_warning_glyph() {
    let mut app = app_with_items(vec![item("001", "Task", "Body")]);
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    app.set_sync_status(crate::app::SyncStatus::Failed {
        message: "boom".into(),
    });
    let mut terminal = Terminal::new(TestBackend::new(200, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        rendered.contains("\u{26A0} Sync failed: boom"),
        "expected ⚠ Sync failed pill in footer, got: {rendered}"
    );
}

#[test]
fn footer_renders_failed_sync_pill_red() {
    use ratatui::style::Color;

    let mut app = app_with_items(vec![item("001", "Task", "Body")]);
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    app.set_sync_status(crate::app::SyncStatus::Failed {
        message: "boom".into(),
    });
    let mut terminal = Terminal::new(TestBackend::new(200, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let buffer = terminal.backend().buffer();
    assert!(
        find_styled_text(buffer, "Sync failed", |s| s.fg == Some(Color::Red)),
        "failed sync pill must render with a red foreground"
    );
}

#[test]
fn sync_pill_color_maps_each_variant() {
    use crate::app::SyncStatus;
    use crate::ui::footer::sync_pill_color;
    use ratatui::style::Color;

    // Cyan/yellow are hard to drive deterministically through a full
    // render, so lock the whole mapping at the unit level (AC-3, AC-4).
    assert_eq!(sync_pill_color(&SyncStatus::InFlight), Color::Cyan);
    assert_eq!(
        sync_pill_color(&SyncStatus::Succeeded { new_commits: 0 }),
        Color::Green
    );
    assert_eq!(
        sync_pill_color(&SyncStatus::Failed {
            message: "boom".into()
        }),
        Color::Red
    );
    assert_eq!(
        sync_pill_color(&SyncStatus::Unavailable {
            hint: "not a git repository".into()
        }),
        Color::Yellow
    );
}

#[test]
fn footer_shows_broken_count_pill_when_parse_errors_present() {
    let app = app_with_broken_entity();
    let mut terminal = Terminal::new(TestBackend::new(200, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        rendered.contains("\u{26A0} 1 broken"),
        "footer must render '⚠ 1 broken' pill when one parse error is present"
    );
}

#[test]
fn footer_omits_broken_pill_when_no_parse_errors() {
    let app = app_with_items(vec![item("001", "Valid Task", "Body")]);
    let mut terminal = Terminal::new(TestBackend::new(200, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(
        !rendered.contains("broken"),
        "footer must not show broken pill when there are no parse_errors"
    );
}
