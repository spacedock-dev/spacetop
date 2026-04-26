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
