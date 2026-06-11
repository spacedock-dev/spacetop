use super::*;

// ---- Task 038: worktree marker in task list + diff in preview ----

fn item_with_worktree_source(id: &str, title: &str, body: &str) -> Entity {
    let mut it = item(id, title, body);
    it.worktree_source = Some(PathBuf::from(format!("/tmp/wt/{id}.md")));
    it
}

#[test]
fn task_row_renders_worktree_marker_when_sourced_from_worktree() {
    // AC-1: worktree-sourced row carries the `⎇` marker; main-only row does not.
    let app = app_with_items(vec![
        item("001", "Main task", "Body"),
        item_with_worktree_source("002", "Worktree task", "Body"),
    ]);
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let buffer = terminal.backend().buffer();

    let wt_hits = find_text(buffer, "Worktree task");
    let main_hits = find_text(buffer, "Main task");
    assert!(!wt_hits.is_empty(), "worktree task row should be rendered");
    assert!(!main_hits.is_empty(), "main task row should be rendered");

    // The marker glyph `⎇` (U+2387) must appear on the worktree row only.
    let (wt_x, wt_y) = wt_hits[0];
    let (_, main_y) = main_hits[0];

    // Marker sits immediately before the title — scan a few cells back.
    let marker = '\u{2387}';
    let mut wt_has_marker = false;
    for x in 0..wt_x {
        if buffer[(x, wt_y)].symbol().chars().any(|c| c == marker) {
            wt_has_marker = true;
            break;
        }
    }
    assert!(wt_has_marker, "worktree row should contain ⎇ marker");

    let mut main_has_marker = false;
    for x in 0..buffer.area.width {
        if buffer[(x, main_y)].symbol().chars().any(|c| c == marker) {
            main_has_marker = true;
            break;
        }
    }
    assert!(!main_has_marker, "main-only row must NOT contain ⎇ marker");
}

#[test]
fn preview_renders_diff_when_main_body_present() {
    // AC-3: when `main_body.is_some()`, the preview body area shows a
    // unified diff with `+`/`-` lines from the divergent content.
    let mut it = item("050", "Divergent", "alpha\nNEW LINE\ngamma\n");
    it.main_body = Some("alpha\nOLD LINE\ngamma\n".to_string());
    let app = app_with_items(vec![it]);
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let buffer = terminal.backend().buffer();
    let text = buffer_text(buffer);

    assert!(
        text.contains("+NEW LINE"),
        "preview should contain '+NEW LINE' from diff; rendered: {text}"
    );
    assert!(
        text.contains("-OLD LINE"),
        "preview should contain '-OLD LINE' from diff; rendered: {text}"
    );
}

#[test]
fn preview_falls_back_to_body_when_main_body_none() {
    // AC-3: when `main_body` is None, the preview renders the body as
    // plain markdown (no `+`/`-` diff prefix lines).
    let app = app_with_items(vec![item("051", "Plain", "alpha\nbeta\ngamma\n")]);
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");
    let buffer = terminal.backend().buffer();
    let text = buffer_text(buffer);

    assert!(
        text.contains("beta"),
        "preview should still render body content"
    );
    // No diff prefixes should leak when main_body is None.
    assert!(
        !text.contains("+beta") && !text.contains("-beta"),
        "preview should not show diff markers when main_body is None"
    );
}
