---
id: "019"
title: Adopt ratskin for preview pane markdown rendering
status: design
source: captain (follows 017 survey and 018 ratatui upgrade)
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

Task 017 identified `ratskin` 0.3.1 as the correct bridge between termimad's `MadSkin` styling and Ratatui `Vec<Line>`. Task 018 upgraded ratatui to 0.30 and confirmed ratskin is compatible. This task replaces the hand-rolled `pulldown-cmark` event loop in `render_markdown_lines` (`src/ui/mod.rs`) with ratskin, giving the preview pane richer, more correct markdown rendering with less custom code.

## Acceptance criteria

**AC-1 -- ratskin is added to Cargo.toml and the project compiles.**
Verified by: `grep ratskin Cargo.toml` confirms the dependency; `cargo check` exits 0.

**AC-2 -- The preview pane renders fenced code blocks with distinct styling using ratskin.**
Verified by: visual inspection of a task entity with a fenced code block; rendered with styled background, no raw backtick fences visible.

**AC-3 -- Multi-line code blocks render each source line on a distinct row.**
Verified by: unit or TestBackend test asserting distinct Y rows for a multi-line fenced block.

**AC-4 -- Inline code, headings, bold, italic, and plain prose render correctly.**
Verified by: the existing preview rendering test suite passes; `cargo test --lib` exits 0.

**AC-5 -- The hand-rolled pulldown-cmark event loop in render_markdown_lines is replaced (not duplicated).**
Verified by: `render_markdown_lines` no longer contains `pulldown-cmark` event handling; pulldown-cmark dependency may be removed from Cargo.toml if no longer needed elsewhere.
