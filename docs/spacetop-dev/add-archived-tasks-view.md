---
id: 006
title: Show completed (archived) tasks in the TUI
status: design
source: captain feedback after build-initial-tui-overview
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

The current overview lists only active tasks. Users want visibility into completed tasks archived under `_archive/` — e.g., to inspect what shipped, review old stage reports, or audit verdicts. This task adds a way to view archived entities from the TUI.

Open design questions to resolve in the `design` stage:

- Unified list with a status filter (active / archived / all) vs. a separate archived screen/tab?
- Key binding for toggling the view.
- How does archived task rendering differ from active (verdict badge, completed timestamp, muted styling)?
- Parser responsibility: should `WorkflowSnapshot` load archives by default, on demand, or via a flag on the loader?
- Preview pane behavior on archived tasks (same fields, plus `completed` / `verdict`).

## Acceptance criteria

_To be firmed up during design. Expected shape:_

**AC-1 -- Users can browse archived tasks without leaving the TUI.**
Verified by: render test asserts archived task titles appear in the selected view mode; key binding toggles the mode.

**AC-2 -- Archived tasks show verdict and completion timestamp in the preview pane.**
Verified by: render test against `_archive/*.md` fixtures.

**AC-3 -- Opening the TUI still defaults to the active-task view; archived view is opt-in.**
Verified by: default-state test.
