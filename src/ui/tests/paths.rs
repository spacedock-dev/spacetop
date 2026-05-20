use super::*;

#[test]
fn fit_path_to_width_keeps_short_path_unchanged() {
    let s = fit_path_to_width("039-foo.md", 40);
    assert_eq!(s, "039-foo.md");
}

#[test]
fn fit_path_to_width_truncates_long_path_with_leading_ellipsis() {
    let long = "/repo/.worktrees/SLUG/docs/spacetop-dev/039-open-entity-file-from-preview.md";
    let s = fit_path_to_width(long, 40);
    // pane=40, label="path: "=6, available=34, so the result is 34 chars
    // (1 ellipsis + 33 tail).
    assert_eq!(s.chars().count(), 34);
    assert!(s.starts_with('\u{2026}'));
    // Truncate from the LEFT so the END of the path stays visible —
    // important because the basename carries the identifying info.
    assert!(
        s.ends_with("from-preview.md"),
        "truncated path should keep the trailing portion of the basename; got {s:?}"
    );
}

#[test]
fn fit_path_to_width_keeps_basename_when_room_is_ample() {
    // With a wider pane the entire basename fits even when the path is
    // truncated, so the user can identify the file at a glance.
    let long = "/repo/.worktrees/SLUG/docs/spacetop-dev/039-open-entity-file-from-preview.md";
    let s = fit_path_to_width(long, 80); // available = 74
    assert!(
        s.starts_with('\u{2026}'),
        "should still mark truncation; got {s:?}"
    );
    assert!(
        s.ends_with("039-open-entity-file-from-preview.md"),
        "with ample pane width the full basename should remain visible; got {s:?}"
    );
}

#[test]
fn fit_path_to_width_collapses_to_ellipsis_when_pane_is_tiny() {
    let s = fit_path_to_width("any/path.md", 6); // label uses all the width
    assert_eq!(s, "\u{2026}");
}

/// Regression for cycle-1 review feedback on 039: the preview header's
/// `path:` line rendered visually EMPTY when the entity was a
/// worktree-resident copy and the absolute fallback path was longer than
/// the preview pane width. The header paragraph wraps with
/// `Wrap { trim: true }`, which word-wraps at the single space between
/// the label and a long path — leaving the label alone on one row and
/// the value on the next. This test exercises BOTH cases:
///   (i) in-workflow-root items → relative path expected, fits inline.
///   (ii) out-of-root (worktree-resident) items → absolute fallback, but
///        truncated with a leading ellipsis so it still fits on one row
///        and the basename stays visible.
/// Either way, the rendered row that begins with "path:" must carry a
/// non-empty visible value.
#[test]
fn path_line_stays_visible_for_long_paths() {
    use crate::domain::{StageDefinition, WorkflowDefinition, WorkflowSnapshot};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let workflow_dir = PathBuf::from("/repo/docs/wf");

    // (i) In-root item: path is /repo/docs/wf/039-foo.md, workflow_dir is
    //     /repo/docs/wf, so strip_prefix yields "039-foo.md" — short and
    //     visible on the same row as the "path:" label.
    let in_root = WorkItem {
        path: PathBuf::from("/repo/docs/wf/039-foo.md"),
        id: "039".to_string(),
        title: "In root".to_string(),
        status: "design".to_string(),
        source: Some("x".to_string()),
        started: None,
        completed: None,
        verdict: None,
        score: None,
        worktree: None,
        issue: None,
        pr: None,
        body: "Body".to_string(),
        worktree_source: None,
        main_body: None,
    };

    // (ii) Out-of-root item: a worktree-resident copy whose absolute path
    //      is well over the preview pane width — exercises the absolute
    //      fallback + width-fit truncation.
    let mut out_of_root = in_root.clone();
    out_of_root.id = "040".to_string();
    out_of_root.title = "Out of root".to_string();
    out_of_root.path = PathBuf::from(
        "/repo/.worktrees/spacedock-ensign-039-open-entity-file-from-preview/docs/wf/040-bar.md",
    );

    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: workflow_dir.clone(),
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
        items: vec![in_root, out_of_root],
    };
    let mut app = App::from_snapshot(workflow_dir.clone(), snapshot);
    // Open the preview on the first (in-root) item.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let mut terminal =
        Terminal::new(TestBackend::new(120, 30)).expect("test terminal should be created");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let buffer = terminal.backend().buffer();
    assert_path_row_non_empty(buffer, "in-root");
    let rendered = buffer_text(buffer);
    assert!(
        rendered.contains("path: 039-foo.md"),
        "in-root item should render relative path on the same row, got: {rendered}"
    );

    // Now move down to the out-of-root item; preview follows the
    // selection.
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");
    let buffer = terminal.backend().buffer();
    assert_path_row_non_empty(buffer, "out-of-root");
    // The truncated absolute path must still surface the basename so the
    // user can identify the file at a glance.
    let rendered = buffer_text(buffer);
    assert!(
        rendered.contains("040-bar.md"),
        "out-of-root item should still show the basename in the path row, got: {rendered}"
    );
    // The leading ellipsis marker is the truncation signal.
    assert!(
        rendered.contains('\u{2026}'.to_string().as_str()),
        "out-of-root long path should be truncated with a leading ellipsis, got: {rendered}"
    );
}

/// Helper: locate the row whose first non-empty content begins with
/// "path:" and assert that some visible character follows the label on
/// the same row. Fails the test with a helpful message otherwise.
fn assert_path_row_non_empty(buffer: &ratatui::buffer::Buffer, label: &str) {
    let hits = find_text(buffer, "path:");
    assert!(
        !hits.is_empty(),
        "({label}) expected to find a 'path:' label in the rendered preview"
    );
    let (x, y) = hits[0];
    let after = x + "path:".chars().count() as u16;
    let mut non_empty_seen = false;
    for col in after..buffer.area.width {
        let cell = &buffer[(col, y)];
        let sym = cell.symbol();
        if sym.is_empty() {
            continue;
        }
        // Skip the single space between label and value.
        if sym == " " {
            continue;
        }
        non_empty_seen = true;
        break;
    }
    let row_text: String = (0..buffer.area.width)
        .map(|cx| buffer[(cx, y)].symbol().to_string())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        non_empty_seen,
        "({label}) 'path:' row at y={y} has no visible value after the label; \
         row text: {row_text:?}"
    );
}
