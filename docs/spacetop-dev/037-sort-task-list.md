---
id: 037
title: Sort the task list by ID or by status
status: plan
source: captain request 2026-05-12
score:
worktree:
issue:
pr:
started: 2026-05-12T07:16:26Z
---

The SpaceTop overview lists tasks in whatever order discovery happens to return. Users should be able to choose the sort order so they can scan the list either by stable identifier or by where each task currently sits in the workflow. Provide at minimum two sort modes — sort by `id` and sort by `status` (stage order as declared in the workflow README) — with a clear way to toggle between them from the TUI and a visible indicator of the active sort.

## Acceptance criteria

**AC-1 -- Task list can be sorted by ID.**
Verified by: a unit/integration test in `src/app.rs` (or wherever overview state lives) that asserts the rendered task order matches ascending `id` after selecting the ID sort mode, across a fixture with mixed-status tasks.

**AC-2 -- Task list can be sorted by status (workflow stage order).**
Verified by: a test asserting the rendered task order groups tasks by status using the stage ordering declared in the workflow README (not alphabetical on the status string), with a documented tiebreaker (e.g., ID ascending) for tasks sharing a status.

**AC-3 -- The active sort mode is toggleable from the TUI and visible to the user.**
Verified by: a keybinding documented in the overview UI (and reflected in `src/ui/`) cycles between sort modes, and the header or status line shows which mode is active. Snapshot-style assertion or rendered-string test confirms the indicator changes when the mode changes.

**AC-4 -- Sort behavior is read-only and does not mutate workflow files.**
Verified by: the sort is implemented purely in app state / rendering with no writes to `docs/spacetop-dev/`; covered by the existing read-only invariant tests or a new assertion that no filesystem writes occur on sort toggle.
