---
id: "016"
title: Fix preview pane markdown code block rendering
status: design
source: captain
started: 2026-04-25T15:19:40Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

The preview pane currently renders markdown code blocks (fenced with triple backticks) as plain text instead of styled code blocks. Users inspecting workflow entity bodies and stage reports see raw backtick fences rather than properly highlighted or visually distinct code sections.

## Example rendering content

Use this section to verify preview pane rendering while working on the fix. The preview of this entity body should display all of the following correctly.

### Fenced code block (Rust)

```rust
fn render_preview(text: &str) -> Vec<Line> {
    let parser = Parser::new(text);
    parser.map(|event| match event {
        Event::Code(s) => Line::styled(s.to_string(), Style::default().fg(Color::Cyan)),
        Event::Text(s) => Line::raw(s.to_string()),
        _ => Line::raw(""),
    }).collect()
}
```

### Fenced code block (shell)

```bash
cargo test --lib -- preview::tests
cargo run -- --workflow-dir docs/spacetop-dev
```

### Fenced code block (long line, 90+ chars)

```
status: design | worktree: .worktrees/spacedock-ensign-016-fix-preview-markdown-code-blocks
```

### Inline code in prose

The `render_preview` function lives in `src/preview.rs`. Pass a `&str` slice and it returns a `Vec<Line<'_>>` ready for the `Paragraph` widget.

### Mixed content (headings, list, bold, italic, code)

The fix touches two areas:

1. **Parser** — swap `Event::Text` handling to detect code fences via `Event::Start(Tag::CodeBlock(_))` and `Event::End(Tag::CodeBlock(_))`.
2. **Renderer** — apply a distinct `Style` (e.g., `bg(Color::DarkGray)`) to lines inside a code block.

Use *pulldown-cmark* events rather than regex so edge cases like nested backticks are handled correctly.

## Acceptance criteria

**AC-1 -- Code fences render as distinct code blocks.**
Fenced code blocks (` ``` ` ... ` ``` `) in entity body markdown are rendered with a visually distinct style (e.g., different background or border) in the preview pane, not as raw backtick text.
Verified by: manual inspection of a task entity containing a fenced code block in the preview pane; the backtick fences are not visible to the user.

**AC-2 -- Inline code renders correctly.**
Inline code spans (single backticks) are rendered distinctly from surrounding prose text.
Verified by: visual check on an entity body containing inline code.

**AC-3 -- Existing text rendering is unaffected.**
Non-code markdown content (headings, lists, bold, italics, plain paragraphs) continues to render correctly after the fix.
Verified by: review of an entity body with mixed content in the preview pane.

**AC-4 -- No TUI layout breakage.**
The preview pane does not overflow or misalign when displaying code blocks of varying line lengths.
Verified by: test with a code block containing a long line (80+ chars).
