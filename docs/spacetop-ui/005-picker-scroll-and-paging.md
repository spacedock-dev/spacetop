---
id: "005"
title: Workflow picker supports scrolling and PageUp/PageDown
status: review
source: captain
started: 2026-04-27T05:07:19Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-005-picker-scroll-and-paging
issue:
pr: #25
mod-block: merge:pr-merge
---

When the workflow picker popup lists more workflows than fit on the screen, the user has no way to see or reach the items below the visible window. The picker should scroll its list as the selection moves past the viewport edge, render a scrollbar so the user can see their position in the list, and respond to PageUp / PageDown for fast navigation through long lists.

Reference: `src/ui/picker.rs` (currently renders the workflow list as a single Paragraph) and `src/app/picker.rs` / `PickerState` for selection state. Existing list selection handling lives near the picker's keyboard input path in `src/app.rs`.

## Acceptance criteria

**AC-1 — Selection scrolls the visible window.**
Verified by: a unit test that constructs a `PickerState` with N workflows, simulates moving the selection past a viewport of height H < N, and asserts the rendered list shifts so the selected row stays visible (top and bottom edges).

**AC-2 — Scrollbar reflects position when the list overflows.**
Verified by: a render-level test (using ratatui's `TestBackend`) that renders the picker against a small area with N > H workflows and asserts a scrollbar is drawn whose thumb position matches the selected index proportion. The scrollbar is omitted when N ≤ H.

**AC-3 — PageUp / PageDown jump by viewport height.**
Verified by: a unit test driving the picker key handler with PageDown / PageUp events and asserting the selected index advances/retreats by approximately the viewport height (clamped to list bounds), without panicking on empty lists or single-item lists.
