---
id: "016"
title: Fix preview pane markdown code block rendering
status: design
source: captain
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

The preview pane currently renders markdown code blocks (fenced with triple backticks) as plain text instead of styled code blocks. Users inspecting workflow entity bodies and stage reports see raw backtick fences rather than properly highlighted or visually distinct code sections.

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
