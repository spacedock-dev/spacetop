---
id: "001"
title: Add workflow title header bar
status: done
source: commission seed — SpaceTop Wireframes dir1-classic.jsx
started: 2026-04-25
completed: 2026-04-25
verdict: PASSED
score: 0.9
worktree:
issue:
pr:
archived: 2026-04-25T09:51:19Z
---

The current dashboard has no standalone title bar. The "classic refined" design (Direction 1) adds a single-line header row **above the stage graph** showing: a muted `Workflow` label, an `[active]`/`[archived]` scope badge, an `archived: N (press a)` hint, and the workflow directory path dimmed.

Matches the existing test `active_view_header_shows_scope_and_archived_placeholder` which already asserts `[active]` and `(press a)` appear in the render.

## Design spec (from dir1-classic.jsx)

```
Workflow  [active]  archived: (press a)  /path/to/workflow/dir
```

- `Workflow` — muted/dim style
- `[active]` or `[archived]` — colored badge: Yellow for active, dim for archived; bold
- `archived: N (press a)` — show archived item count if known, else `(press a)` hint
- path — dim, rightmost, can be truncated if too long

This is a single text line rendered in a `Constraint::Length(1)` row inserted **before** the graph ribbon in the vertical layout.

## Implementation notes

In `render_overview` (`src/ui/mod.rs`):
- Add a `Constraint::Length(1)` row to the vertical layout constraints before the graph row
- Implement `render_header_bar(frame, header_area, state, session)` that builds a single `Line` with `Span`s:
  1. `Span::styled("Workflow ", dim)`
  2. `Span::styled("[active]", yellow+bold)` or `Span::styled("[archived]", dim+bold)`
  3. `Span::raw("  archived: ")` + count or `Span::styled("(press a)", dim)`
  4. `Span::styled(path_str, dim)` — use `state.workflow_dir.display()`

The existing `active_view_header_shows_scope_and_archived_placeholder` test already covers the `[active]` + `(press a)` assertions, so this change should make it pass with the same assertions.

## Acceptance criteria

**AC-1 — Header row renders `[active]` or `[archived]` scope badge.**
Verified by: `cargo test active_view_header_shows_scope_and_archived_placeholder` passes.

**AC-2 — Header row renders `(press a)` archived toggle hint in active view.**
Verified by: same test above.

**AC-3 — All existing render tests still pass.**
Verified by: `cargo test` exits 0.

**AC-4 — The "Workflow" label appears at the start of the header row.**
Verified by: new test or assertion that rendered buffer contains `Workflow` in the first 2 rows of the dashboard area.

## Stage Report: implement

- DONE: Add `Constraint::Length(1)` row before graph ribbon in `render_overview`
  New layout: [header_bar(1), graph(7), content(min), footer(1)].
- DONE: Implement `render_header_bar` function with scope badge and archived hint
  Renders `[active]` (yellow+bold) or `[archived]` (dim+bold), `(press a)` hint, dim path, trailing `─` fill.
- DONE: `active_view_header_shows_scope_and_archived_placeholder` passes
  `cargo test` 110/110 passed; commit 548a7f5.
- DONE: `dashboard_pane_spans_full_terminal_width` preserved
  Header bar fills full width using `─` separator characters so (0,0) and (width-1,0) are non-blank.
- DONE: All existing render tests still pass
  110 tests pass; 0 failed.

### Summary

Added a single-line header bar above the stage graph ribbon in `render_overview`. The bar shows `Workflow` label, `[active]`/`[archived]` scope badge, `(press a)` archived hint, and the workflow directory path. Trailing `─` characters fill the remaining width to keep the `dashboard_pane_spans_full_terminal_width` test passing. The header bar row is `Constraint::Length(1)` inserted as the first constraint in the vertical layout.
