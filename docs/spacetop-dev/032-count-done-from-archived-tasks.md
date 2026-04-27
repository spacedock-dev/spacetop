---
id: 032
title: Count done from archived tasks in workflow overview
status: review
source: user request 2026-04-27
score: 0.4
worktree: .worktrees/spacedock-ensign-032-count-done-from-archived-tasks
issue:
pr:
started: 2026-04-27T02:26:48Z
---

The workflow overview should not show a nonzero `#done` count for `docs/spacetop-dev` just because completed items exist in `_archive/`. When the workflow is shown in its active scope, `#done` should be `0`.

## Acceptance criteria

**AC-1 -- Active overview done count stays at zero.**
Verified by: the workflow overview for `docs/spacetop-dev` renders `done: 0` in the stage summary while active items remain unchanged.

**AC-2 -- Archived items do not affect active stage totals.**
Verified by: archived tasks are still available in archived scope, but they do not contribute to the active stage count shown in the workflow header.

## Implementation Plan

1. Keep the data boundary in `src/app/overview.rs` and `src/parser/snapshot.rs` explicit: active stage counts must continue to come from `WorkflowSnapshot.items`, while `_archive/` remains a separate cache populated only by `load_archived_items()`.
2. Verify the workflow loader in `src/parser/snapshot.rs` never folds archived paths into the active snapshot. If a regression exists, fix it there rather than in the UI so the parser remains the single source of truth for what is active.
3. Reconfirm `OverviewState::stage_counts()` in `src/app/overview.rs` is counting only `snapshot.items` and not `archived_items`. If any helper currently mixes the scopes, split it so active overview totals are scope-local.
4. Keep `src/ui/graph.rs` as a pure renderer over `OverviewState::stage_counts()` and `view_scope()`. The fix should not special-case `_archive/` in the TUI layer; the UI should just render the already-correct active counts.
5. Add a regression test in `src/parser/tests.rs` that loads a fixture with active items plus `_archive/done.md` and asserts the active snapshot does not gain an extra `done` item.
6. Add a state-level regression in `src/app/tests.rs` asserting `App::load()` reports `stage_counts()` with `done == 0` while archived scope still becomes available after toggling `a`.
7. Add a renderer-level regression in `src/ui/graph.rs` verifying the workflow graph still shows the active scope header and count row, and that the `done` count remains zero until an active `done` item actually exists.

## Test Strategy

- Parser coverage: prove archive files are ignored by the active snapshot loader and still load through `load_archived_items()` only.
- App-state coverage: prove `stage_counts()` is derived from active snapshot items and does not change when archived entries exist.
- UI coverage: prove the graph renders the active scope counts without inventing a `done` total from archived items.

## Verification Commands

```bash
cargo test parser::tests::loads_workflow_snapshot_from_directory_ignoring_mods_and_archive
cargo test app::tests::toggle_scope_key_a_flips_to_archived_and_loads_lazily
cargo test ui::graph::counts_row_aligns_under_nodes_and_marks_active_stage ui::graph::header_row_contains_scope_label_and_archived_count_only
make lint
```

## Stage Report: plan

- DONE: Produce a concrete step-by-step implementation plan for the bug fix, with the module boundary called out clearly.
  Added an implementation plan that assigns ownership across `src/parser/snapshot.rs`, `src/app/overview.rs`, and `src/ui/graph.rs`.
- DONE: Name the focused test strategy and the exact verification commands that should prove the change.
  Added parser, app-state, and UI coverage notes plus concrete `cargo test` and `make lint` commands.
- DONE: Keep the work scoped to the task body and append a `## Stage Report: plan` section that accounts for every checklist item with DONE / SKIPPED / FAILED.
  Appended only body content to `/Users/kent/Dev/InfuseAI/GitHub/spacetop/docs/spacetop-dev/032-count-done-from-archived-tasks.md`; frontmatter was left untouched.

### Summary

This stage is a planning pass only. The task body now contains a scoped implementation plan, test strategy, and verification commands for preventing archived `done` items from inflating the active workflow overview.
