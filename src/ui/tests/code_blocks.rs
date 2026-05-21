use super::*;

#[test]
fn preview_renders_fenced_code_block_without_backtick_fences() {
    let body = "Some prose.\n\n```rust\nlet x = 1;\n```\n\nAfter block.";
    let app = app_with_items(vec![item("001", "Code Block Preview", body)]);
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");

    let buffer = terminal.backend().buffer();
    let rendered = buffer_text(buffer);

    // Backtick fences must not appear
    assert!(
        !rendered.contains("```"),
        "backtick fences should not be visible"
    );

    // Code body text must appear
    assert!(
        rendered.contains("let x = 1;"),
        "code body text must be rendered"
    );

    // Code text must carry distinct styling (Cyan fg or DarkGray bg)
    assert!(
        find_styled_text(buffer, "let x = 1;", |style| {
            style.fg == Some(Color::Cyan) || style.bg == Some(Color::DarkGray)
        }),
        "code block text must have distinct style"
    );
}

#[test]
fn render_markdown_termimad_multiline_code_block_emits_one_line_per_source_line() {
    // Termimad fills a code block to the outer width with a Cyan/DarkGray
    // slab. Each source line should remain on its own Line, preserving the
    // visible text at the start of the styled span and padding out to at
    // least the requested pane width.
    let pane_width: u16 = 40;
    let markdown = "```rust\nlet x = 1;\nlet y = 2;\nlet z = 3;\n```";
    let lines = super::markdown::render_markdown_termimad(markdown, pane_width);

    // Collect spans that carry the slab styling.
    let code_spans: Vec<&str> = lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .filter_map(|span| {
            if span.style.bg == Some(Color::DarkGray) && span.style.fg == Some(Color::Cyan) {
                Some(span.content.as_ref())
            } else {
                None
            }
        })
        .collect();

    // There must be exactly 3 code spans, one per source line.
    assert_eq!(
        code_spans.len(),
        3,
        "each source line in a multi-line code block must produce a separate styled span; got {:?}",
        code_spans,
    );

    // Termimad pads each code compound to at least the outer width.
    for span_content in &code_spans {
        assert!(
            span_content.chars().count() >= pane_width as usize,
            "code line span must be padded to at least pane_width ({pane_width}), got len {}",
            span_content.chars().count(),
        );
    }

    // Source text must be preserved at the start of the padded span.
    assert!(
        code_spans[0].starts_with("let x = 1;"),
        "first code line content must be preserved (got {:?})",
        code_spans[0],
    );
    assert!(
        code_spans[1].starts_with("let y = 2;"),
        "second code line content must be preserved (got {:?})",
        code_spans[1],
    );
    assert!(
        code_spans[2].starts_with("let z = 3;"),
        "third code line content must be preserved (got {:?})",
        code_spans[2],
    );
}

#[test]
fn preview_renders_multiline_code_block_on_distinct_rows() {
    // This test renders a 3-line fenced code block through the full TUI
    // pipeline (render_markdown_lines -> Paragraph widget -> TestBackend)
    // and asserts that each code line appears at a different Y coordinate
    // in the terminal buffer.  A regression where all code lines collapse
    // to a single row would fail this assertion even if the unit test for
    // render_markdown_lines passes.
    let body = "```rust\nlet x = 1;\nlet y = 2;\nlet z = 3;\n```";
    let app = app_with_items(vec![item("001", "Multi-line Code Block", body)]);
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).expect("terminal");
    terminal
        .draw(|frame| render(frame, &app))
        .expect("render should succeed");

    let buffer = terminal.backend().buffer();
    let rendered = buffer_text(buffer);

    // All three code lines must appear somewhere in the rendered output.
    assert!(
        rendered.contains("let x = 1;"),
        "first code line must appear"
    );
    assert!(
        rendered.contains("let y = 2;"),
        "second code line must appear"
    );
    assert!(
        rendered.contains("let z = 3;"),
        "third code line must appear"
    );

    // Each code line must be on a distinct row in the buffer.
    let y_x = find_text(buffer, "let x = 1;");
    let y_y = find_text(buffer, "let y = 2;");
    let y_z = find_text(buffer, "let z = 3;");

    assert!(!y_x.is_empty(), "let x = 1; not found in buffer");
    assert!(!y_y.is_empty(), "let y = 2; not found in buffer");
    assert!(!y_z.is_empty(), "let z = 3; not found in buffer");

    let row_x = y_x[0].1;
    let row_y = y_y[0].1;
    let row_z = y_z[0].1;

    assert_ne!(
        row_x, row_y,
        "first and second code lines must render on different rows (got row {row_x})"
    );
    assert_ne!(
        row_y, row_z,
        "second and third code lines must render on different rows (got row {row_y})"
    );
    assert!(
        row_y > row_x,
        "second code line (row {row_y}) must be below first (row {row_x})"
    );
    assert!(
        row_z > row_y,
        "third code line (row {row_z}) must be below second (row {row_y})"
    );
}

#[test]
fn code_block_background_fills_pane_width_in_wrap_mode() {
    let body = "```rust\nlet x = 1;\n```";
    let app = app_with_items(vec![item("001", "Code Wrap", body)]);
    // app_with_items presses Enter to open preview; wrap is on by default.

    let width: u16 = 80;
    let height: u16 = 24;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");

    let buffer = terminal.backend().buffer();
    // Find the row that contains the code body text.
    let hits = find_text(buffer, "let x = 1;");
    assert!(!hits.is_empty(), "code line must appear in buffer");
    let (_, row) = hits[0];

    // Every cell on that row within the preview pane content area must have
    // DarkGray background so the code block background fills the full width.
    // The preview pane has a LEFT border at width/2, so content starts at width/2+1.
    let preview_start = width / 2 + 1;
    for col in preview_start..width {
        let style = buffer[(col, row)].style();
        assert_eq!(
            style.bg,
            Some(Color::DarkGray),
            "col {col} on code row {row} must have DarkGray background in wrap mode"
        );
    }
}

#[test]
fn code_block_long_line_both_wrapped_rows_have_full_background() {
    // Code line longer than the preview pane width so it wraps to a second row.
    let long_line = "X".repeat(120);
    let body = format!("```rust\n{long_line}\n```");
    let app = app_with_items(vec![item("001", "Long Code Wrap", &body)]);
    // wrap is on by default.

    let width: u16 = 80;
    let height: u16 = 30;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");

    let buffer = terminal.backend().buffer();
    // Find the first row containing the leading X run.
    let hits = find_text(buffer, "XXXX");
    assert!(!hits.is_empty(), "long code line must appear in buffer");
    let (_, first_row) = hits[0];

    // Both the first and second visual row of the wrapped code line must be
    // fully backgrounded. Check rightmost and leftmost preview content cells on each row.
    // The preview pane has a LEFT border at width/2, so content starts at width/2+1.
    let preview_start = width / 2 + 1;
    for row in [first_row, first_row + 1] {
        let style_right = buffer[(width - 1, row)].style();
        assert_eq!(
            style_right.bg,
            Some(Color::DarkGray),
            "rightmost cell on wrapped code row {row} must have DarkGray background"
        );
        let style_left = buffer[(preview_start, row)].style();
        assert_eq!(
            style_left.bg,
            Some(Color::DarkGray),
            "preview_start cell on wrapped code row {row} must have DarkGray background"
        );
    }
}

#[test]
fn code_block_background_fills_width_when_scrollbar_is_shown() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    // Build a body tall enough to trigger the vertical scrollbar, then a code block.
    // This is the exact case the two-pass fix addresses: body_area is 1 column
    // narrower than body_inner when the scrollbar is present.
    let many_lines = "line\n".repeat(40);
    let body = format!("{many_lines}```rust\nlet x = 1;\n```");
    let mut app = app_with_items(vec![item("001", "Scrollbar Code", &body)]);
    // wrap is on by default.

    let width: u16 = 80;
    let height: u16 = 24;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");

    // First draw: populates max_preview_scroll so PageDown can scroll.
    terminal
        .draw(|frame| render(frame, &app))
        .expect("first render");

    // Scroll to the bottom using PageDown repeatedly.
    for _ in 0..20 {
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        terminal
            .draw(|frame| render(frame, &app))
            .expect("scroll render");
    }

    let buffer = terminal.backend().buffer();
    let hits = find_text(buffer, "let x = 1;");
    assert!(
        !hits.is_empty(),
        "code line must be visible after scrolling to bottom"
    );
    let (_, row) = hits[0];

    // With the scrollbar present the content area is body_inner.width - 1 columns wide.
    // The rightmost content cell (width - 2; width - 1 is the scrollbar column) must
    // have DarkGray background.
    let preview_start = width / 2 + 1;
    for col in preview_start..width - 1 {
        let style = buffer[(col, row)].style();
        assert_eq!(
            style.bg,
            Some(Color::DarkGray),
            "col {col} on code row {row} must have DarkGray background with scrollbar present"
        );
    }
}
