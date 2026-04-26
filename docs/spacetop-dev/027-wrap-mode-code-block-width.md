---
id: "027"
title: "Code block background does not fill full pane width in wrap mode"
status: plan
source: bug report
started: 2026-04-26T04:47:04Z
completed:
verdict:
score: 0.7
worktree:
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
