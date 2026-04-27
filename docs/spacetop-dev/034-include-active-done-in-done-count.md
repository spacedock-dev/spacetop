---
id: "034"
title: Include active done items in workflow done count
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

The workflow overview's `done` count currently equals `archived_done_count` whenever the archive cache is populated. If a task has `status: done` on the active workflow (terminal stage reached, but the entity has not yet been moved into `_archive/`), it does not contribute to `#done`. The count should reflect every task that has reached the terminal stage, regardless of whether the entity file has been relocated.

This is the inverse of task 032: 032 made sure archived items did not double-count when also present as active items, but it did so by replacing the active count with the archived count for the `done` stage instead of summing the two disjoint sources.

Reference: `src/app/overview.rs::stage_counts` (around lines 324–331) — the `done` branch overwrites `count.items` with `archived_done_count` instead of adding the active terminal-stage items.

## Acceptance criteria

**AC-1 — Active terminal items still contribute to `#done`.**
Verified by: a unit test that loads a snapshot with one active item at `status: done` and zero archived items, and asserts `stage_counts()` reports `done == 1`.

**AC-2 — Archived items continue to contribute to `#done`.**
Verified by: the existing `stage_counts_include_archived_done_items_from_the_workflow_archive` test in `src/app/tests.rs` continues to pass.

**AC-3 — Active and archived done items sum without double-counting.**
Verified by: a unit test with N active `done` items and M archived `done` items (entities in `_archive/` are not also present in the active snapshot) asserting `stage_counts()` reports `done == N + M`.
