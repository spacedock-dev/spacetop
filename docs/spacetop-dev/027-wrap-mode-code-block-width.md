---
id: "027"
title: "Code block background does not fill full pane width in wrap mode"
status: implement
source: bug report
started: 2026-04-26T04:47:04Z
completed:
verdict:
score: 0.7
worktree: .worktrees/spacedock-ensign-027-wrap-mode-code-block-width
issue:
pr:
---

When word-wrap mode is enabled in the preview pane (task 022, key `w`), fenced code blocks render with a background that only spans the width of the code text rather than the full pane width. In no-wrap mode code blocks correctly fill the available width. The truncated background makes code blocks visually inconsistent and harder to read.

## Root cause area

The preview pane renders markdown using `pulldown-cmark`. Code blocks are rendered as `Paragraph` widgets with a styled background. In no-wrap mode each line is padded to the full pane width. In wrap mode the line width is constrained by the wrapped text content, so the background only covers the text characters rather than spanning the full widget width.

The fix likely involves padding each code block line to the full pane width before rendering, or using a `Block` widget background behind the code `Paragraph` to ensure full-width coverage regardless of wrap state.

## Acceptance criteria

**AC-1 -- Code block background fills full pane width in wrap mode.**
When wrap mode is active, fenced code block lines have a background that extends to the right edge of the preview pane, matching the behaviour in no-wrap mode.
Verified by: snapshot test asserting the code block background spans the full widget width in wrap mode.

**AC-2 -- No-wrap mode code block rendering is unchanged.**
Existing code block background behaviour in no-wrap mode is unaffected.
Verified by: existing snapshot tests continue to pass.

**AC-3 -- Wrapped code lines that exceed pane width are also fully backgrounded.**
When a long code line wraps onto a second visual row, both rows have a full-width background.
Verified by: snapshot test with a long code line in wrap mode.

## Stage Report: design

- DONE: Problem statement names the exact render path in src/ui/mod.rs where code block lines are emitted and why wrap mode breaks full-width background.
  `render_markdown_lines` (line 710) pads code block lines to `pane_width` using `format!("{:<width$}", ...)` (line 803), then the caller at line 545 passes `body_inner.width` as `pane_width`. When a scrollbar is shown, `body_area` is 1 column narrower (line 552); the padded span is then wider than the render area and ratatui's `Wrap` engine splits it. On the resulting visual rows, trailing-space background coverage becomes unreliable because ratatui only applies the span style to the characters present, not to the remainder of the cell row.
- DONE: Fix direction is confirmed against the actual rendering code — padding vs Block widget approach.
  The padding approach is architecturally correct. The concrete fix is to defer `render_markdown_lines` until after the `show_scrollbar` / `body_area` decision, then pass `body_area.width` instead of `body_inner.width` as `pane_width`. This ensures padded code lines exactly fill the render area without triggering a wrap. The Block widget alternative would require splitting code blocks into separate `Paragraph` widgets layered over a background widget, which is a larger structural change and is not preferred.

### Summary

The bug lives in `src/ui/mod.rs`: `render_markdown_lines` is called at line 545 with `body_inner.width` before the scrollbar decision narrows `body_area` by 1 column at line 552. Code block lines padded to `body_inner.width` are 1 cell wider than the wrap-mode render area, causing ratatui to split them and leave the background incomplete. The confirmed fix is to move the `render_markdown_lines` call after the `body_area` calculation and pass `body_area.width` as `pane_width`. No Block widget layer is needed.

## Implementation Plan

### File

`src/ui/mod.rs` — single-file change, no new modules or crate dependencies.

### Step 1 — Move the `render_markdown_lines` call

**Context:** In `src/ui/mod.rs`, inside `render_preview` (around line 537–588), the current
sequence is:

```
line 545  let body_lines = render_markdown_lines(&item.body, usize::MAX, body_inner.width as usize);
line 546  let content_height = body_lines.len() as u16;
line 547  let show_scrollbar = content_height > body_inner.height && body_inner.width > 1;
line 548  let body_area = if show_scrollbar { … body_inner.width - 1 … } else { body_inner };
```

**Change:** Delete lines 545–546 from their current position. Insert them after `body_area` is
assigned (after line 557 in the original numbering), and change `body_inner.width` to
`body_area.width`:

```rust
// BEFORE body_area block:
//   (removed) let body_lines = render_markdown_lines(&item.body, usize::MAX, body_inner.width as usize);
//   (removed) let content_height = body_lines.len() as u16;
let show_scrollbar_candidate = body_inner.height; // placeholder — see below

// Scrollbar decision:
let body_area = if /* content overflows */ { … } else { body_inner };

// AFTER body_area block (new location):
let body_lines = render_markdown_lines(&item.body, usize::MAX, body_area.width as usize);
let content_height = body_lines.len() as u16;
```

Because `show_scrollbar` itself depends on `content_height`, and `content_height` now depends on
`body_area.width` (which depends on `show_scrollbar`), a single pass introduces a circular
dependency. The solution is a **two-pass approach**:

1. Call `render_markdown_lines` once with `body_inner.width` to get `content_height` and decide
   `show_scrollbar` / `body_area`.
2. If `show_scrollbar` is true (meaning `body_area.width == body_inner.width - 1`), call
   `render_markdown_lines` again with `body_area.width` to get correctly-padded lines.
3. If `show_scrollbar` is false, `body_area.width == body_inner.width` so the first call's lines
   are already correct — reuse them.

Concretely, the diff in `src/ui/mod.rs` is:

```rust
// --- old (lines 545-557) ---
let body_lines = render_markdown_lines(&item.body, usize::MAX, body_inner.width as usize);
let content_height = body_lines.len() as u16;
let show_scrollbar = content_height > body_inner.height && body_inner.width > 1;
let body_area = if show_scrollbar {
    Rect { …, width: body_inner.width - 1, … }
} else {
    body_inner
};

// --- new ---
// First pass: measure line count with full inner width to decide scrollbar.
let body_lines_full = render_markdown_lines(&item.body, usize::MAX, body_inner.width as usize);
let content_height_full = body_lines_full.len() as u16;
let show_scrollbar = content_height_full > body_inner.height && body_inner.width > 1;
let body_area = if show_scrollbar {
    Rect { …, width: body_inner.width - 1, … }
} else {
    body_inner
};
// Second pass: re-render only when the render width changed (scrollbar is shown).
let body_lines = if show_scrollbar {
    render_markdown_lines(&item.body, usize::MAX, body_area.width as usize)
} else {
    body_lines_full
};
let content_height = body_lines.len() as u16;
```

All subsequent references to `body_lines` and `content_height` remain unchanged.

### Step 2 — Add tests for AC-1, AC-2, and AC-3

Add three tests inside the existing `#[cfg(test)]` block in `src/ui/mod.rs`.

**AC-1 test** — code block background fills full pane width in wrap mode.

Strategy: render a markdown body with a short code block through `TestBackend`, enable wrap mode
via `KeyCode::Char('w')`, then assert that every cell in the code block row has
`bg == Color::DarkGray` all the way to the right edge of the preview pane.

```rust
#[test]
fn code_block_background_fills_pane_width_in_wrap_mode() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::style::Color;

    let body = "```rust\nlet x = 1;\n```";
    let mut app = app_with_items(vec![item("001", "Code Wrap", body)]);
    // Wrap is already opened (app_with_items presses Enter); enable wrap mode.
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));

    let width: u16 = 80;
    let height: u16 = 24;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).expect("render");

    let buffer = terminal.backend().buffer();
    // Find the row that contains "let x = 1;" (the code line).
    let hits = find_text(buffer, "let x = 1;");
    assert!(!hits.is_empty(), "code line must appear in buffer");
    let (_, row) = hits[0];

    // Every cell on that row that is within the preview pane (right half,
    // starting roughly at width/2) must have DarkGray background.
    let preview_start = width / 2;
    for col in preview_start..width {
        let style = buffer[(col, row)].style();
        assert_eq!(
            style.bg,
            Some(Color::DarkGray),
            "col {col} on code row {row} must have DarkGray background in wrap mode"
        );
    }
}
```

**AC-2 test** — existing no-wrap mode tests pass without modification.

No new test is required for AC-2: the existing tests `render_markdown_lines_multiline_code_block_emits_one_line_per_source_line` and `preview_renders_multiline_code_block_on_distinct_rows` already cover no-wrap behaviour and must continue to pass. The verification command confirms this:

```
cargo test render_markdown_lines_multiline_code_block_emits_one_line_per_source_line
cargo test preview_renders_multiline_code_block_on_distinct_rows
```

**AC-3 test** — long code line that wraps still has full-width background on both visual rows.

```rust
#[test]
fn code_block_long_line_both_wrapped_rows_have_full_background() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::style::Color;

    // Code line longer than the preview pane width so it wraps to a second row.
    let long_line = "X".repeat(120);
    let body = format!("```rust\n{long_line}\n```");
    let mut app = app_with_items(vec![item("001", "Long Code Wrap", &body)]);
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));

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
    // fully backgrounded. Check the last cell of the terminal on each row.
    let preview_start = width / 2;
    for row in [first_row, first_row + 1] {
        let style = buffer[(width - 1, row)].style();
        assert_eq!(
            style.bg,
            Some(Color::DarkGray),
            "rightmost cell on wrapped code row {row} must have DarkGray background"
        );
        // Also check at preview_start to confirm coverage on both sides.
        let style_left = buffer[(preview_start, row)].style();
        assert_eq!(
            style_left.bg,
            Some(Color::DarkGray),
            "preview_start cell on wrapped code row {row} must have DarkGray background"
        );
    }
}
```

### Verification commands

Run these from the repository root after applying the change:

```bash
# Unit tests: AC-2 — existing no-wrap tests must still pass
cargo test render_markdown_lines_multiline_code_block_emits_one_line_per_source_line -- --nocapture
cargo test preview_renders_multiline_code_block_on_distinct_rows -- --nocapture

# New tests: AC-1 and AC-3
cargo test code_block_background_fills_pane_width_in_wrap_mode -- --nocapture
cargo test code_block_long_line_both_wrapped_rows_have_full_background -- --nocapture

# Full test suite must stay green
cargo test
```

### Module and file ownership

- Only `src/ui/mod.rs` is modified.
- `render_markdown_lines` remains a private function; its signature (`markdown`, `max_lines`,
  `pane_width`) is unchanged.
- No new public API, no new crate dependencies.

## Stage Report: plan

- DONE: Plan specifies moving render_markdown_lines call to after body_area is computed and passing body_area.width as pane_width.
  Two-pass approach documented: first pass determines show_scrollbar/body_area, second pass re-renders lines only when scrollbar is shown (width changed), all in src/ui/mod.rs.
- DONE: Plan includes snapshot test commands for AC-1, AC-2, and AC-3.
  cargo test commands for all three ACs listed under Verification commands; AC-1 and AC-3 covered by new tests, AC-2 verified by existing tests that must continue to pass.

### Summary

The plan is a two-pass render call in `src/ui/mod.rs`: a first `render_markdown_lines` call with `body_inner.width` resolves the scrollbar decision and `body_area`, then a conditional second call with `body_area.width` produces correctly-padded code block lines when the scrollbar narrows the render area by 1 column. Three new tests cover AC-1 (wrap mode full-width background), AC-3 (long wrapped code line full-width background), and AC-2 is verified by the two existing tests that must continue to pass.
