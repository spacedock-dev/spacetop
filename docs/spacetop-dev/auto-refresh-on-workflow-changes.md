---
id: 008
title: Auto-refresh task list and detail when workflow files change
status: design
source: captain feedback during 006 planning
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

The TUI currently loads `WorkflowSnapshot` once at startup. If another tool (the captain's editor, a concurrent FO session, a dispatched worker) modifies files under the workflow directory, the TUI shows stale data until restart. Watch the workflow directory for file change events and refresh the in-memory snapshot so the task list and the selected task's detail pane update live.

Open design questions to resolve in the `design` stage:

- Watcher crate choice and cost: `notify` (cross-platform inotify/FSEvents/ReadDirectoryChangesW) vs. a polling fallback. Any feature flags needed?
- Event scope — whole workflow dir recursively, or just top-level `*.md` + `_archive/` + folder-form `{slug}/index.md` paths?
- Debounce strategy — editors often emit bursts (write-temp + rename). How long a quiet window before we trigger a reload?
- Event plumbing into the TUI: a background thread sending into the crossterm event channel, or a separate channel polled in the main loop?
- Selection preservation: if the selected task's index shifts (new task added, archive removed), do we keep the same task by slug, keep the same index (clamped), or reset to top?
- Error handling: file deleted mid-render, parse error in a freshly-saved file (half-written frontmatter), watcher backend failure.
- Are archived entities in scope too? (Consistent with task 006 scope.)

## Acceptance criteria

_To be firmed up during design. Expected shape:_

**AC-1 -- Modifying a task file outside the TUI triggers a reload within a bounded time window.**
Verified by: integration test writes to a fixture workflow file; TUI state/snapshot reflects the change within N ms.

**AC-2 -- Selection is preserved across refresh when the underlying task still exists.**
Verified by: app-state test: select task, simulate change event, assert selection stays on the same slug.

**AC-3 -- Parse errors on a single file do not crash the TUI; the prior snapshot is retained.**
Verified by: simulate a half-written file event, assert the TUI keeps rendering the last good snapshot and surfaces a non-fatal error indicator.

**AC-4 -- Watcher is off by default if the platform lacks support, with a clear fallback path.**
Verified by: platform-conditional test or documented behavior.
