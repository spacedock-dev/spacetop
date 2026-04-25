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

## Parser and TUI constraints

### Markdown crate

The project uses `pulldown-cmark = "0.13"` (currently resolved to 0.13.3). The
existing `render_markdown_lines` function in `src/ui/mod.rs` already imports
and uses `Parser`, `Tag`, and `TagEnd` from this crate. The fix must stay
within the same function.

### Relevant pulldown-cmark event sequence for fenced code blocks

```
Start(Tag::CodeBlock(CodeBlockKind::Fenced(info_string)))
Text("line one\n")
Text("line two\n")
End(TagEnd::CodeBlock)
```

Key facts:
- `CodeBlockKind::Fenced(info)` carries the language hint (e.g. `"rust"`,
  `"bash"`, or `""` for unlabeled blocks). The info string is available but
  syntax highlighting is out of scope for this fix.
- `CodeBlockKind::Indented` covers four-space-indented code blocks; the fix
  should treat these the same as fenced blocks.
- Text events inside a code block arrive as raw source lines including their
  trailing `\n`. The `\n` must be stripped and each newline must become a
  separate `Line` rather than appended to the current spans accumulator.
- The existing `MarkdownEvent::Code(text)` arm handles *inline* code spans
  (single backticks); that arm already applies `Color::Yellow` and should be
  left intact (AC-2 is already passing; the fix must not regress it).

### State variable

A boolean `in_code_block: bool` should be added to the local state variables
in `render_markdown_lines`. It is set to `true` on
`Start(Tag::CodeBlock(_))` and reset to `false` on `End(TagEnd::CodeBlock)`.

### Styling approach

Each line of a fenced code block should be emitted as a `Line` whose single
`Span` carries a distinct style. The recommended style is
`Style::default().fg(Color::Cyan).bg(Color::DarkGray)` (or similar — exact
colors are an implementation choice as long as they are visually distinct from
plain prose and pass the AC-1 inspection check). The language hint line (e.g.
` rust` in ```` ```rust ````) is not rendered; only the body text lines are
emitted.

### Block spacing

The code block should be treated as its own block for the purposes of
`add_markdown_block_spacing`. Calling `flush_text_block` before
`Start(Tag::CodeBlock(_))` and after `End(TagEnd::CodeBlock)` ensures a blank
separator row appears between the code block and adjacent content, consistent
with how paragraphs and headings are separated today.

### Horizontal scroll

The existing horizontal scroll implementation (`preview_scroll_x`,
`max_preview_scroll_x`) already tracks `line_width` across all `body_lines`.
Long code lines (AC-4) will automatically participate in horizontal scrolling
without any additional changes to scroll logic.

### Ratatui `Paragraph` widget

The body is rendered via `Paragraph::new(body_lines).scroll(...)` with no
wrapping. This is correct for code blocks: code lines should not word-wrap.
No change to the widget configuration is needed.

### Unit test surface

New tests should be added in the `#[cfg(test)] mod tests` block in
`src/ui/mod.rs`, following the pattern of `preview_renders_markdown_body_instead_of_raw_markers`.
Tests should use `app_with_items(vec![item(...)])` and `TestBackend` renders.
Two tests are expected:
1. Fenced code block text appears without backtick fences; lines use a
   visually distinct style (check `fg` or `bg` color on rendered cells).
2. Inline code (already tested) is unaffected — existing test passes unchanged.

## Stage Report: design

- DONE: Problem statement and target user flow are clearly articulated in the entity body.
  Entity body opens with a clear problem statement (backtick fences rendered as plain text) and includes an example content section usable during implementation.
- DONE: Acceptance criteria cover all rendering cases and each has a concrete verification method.
  Four AC items covering fenced code blocks, inline code, non-code content, and layout breakage; each states a manual or test-based verification method.
- DONE: Parser/TUI constraints relevant to the implementation are named (e.g. which markdown crate, how styled lines map to Ratatui widgets).
  "Parser and TUI constraints" section added: pulldown-cmark 0.13 event sequence for CodeBlock, required state variable, styling approach, block spacing, horizontal scroll, Paragraph widget config, and unit test surface.

### Summary

The entity body already contained a solid problem statement, example content, and four AC items. The design stage added a "Parser and TUI constraints" section that maps the pulldown-cmark 0.13 event API to concrete implementation steps: a boolean `in_code_block` guard, per-line `Line` emission with distinct style, `flush_text_block` calls for block spacing, and two new unit tests. The existing horizontal scroll and `Paragraph` widget setup require no changes. The task is ready for the implement stage.
