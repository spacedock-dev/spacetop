---
id: "034"
title: Include active done items in workflow done count
status: review
source: captain
started: 2026-04-27T04:29:16Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-034-include-active-done-in-done-count
issue:
pr:
mod-block: merge:pr-merge
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

## Implementation Plan

1. **Fix site — `src/app/overview.rs::stage_counts` (lines ~310–333).**
   The current `done` branch (lines 324–331) overwrites `count.items` with `archived_done_count` when the cache is populated. The fix is to **sum** rather than **replace**: keep the active-derived `count.items` (number of `snapshot.items` with `status == "done"`) and add `archived_done_count.unwrap_or(0)` to it. Pseudocode:

   ```rust
   .map(|mut count| {
       if count.name == "done" {
           if let Some(archived_done_count) = self.archived_done_count {
               count.items += archived_done_count;
           }
       }
       count
   })
   ```

   Only this one branch changes. Non-`done` stages stay derived from active `snapshot.items`. The `Option<usize>` cache contract is preserved so test `stage_counts_reuse_cached_archived_done_count_after_archive_disappears` still holds (cached `Some(N)` continues to contribute even if the archive directory is later removed).

2. **Disjointness argument — no double-count risk by construction.**
   - The active snapshot loader (`src/parser/snapshot.rs` via `load_workflow_dir`) walks the workflow directory and explicitly skips `_archive/` (covered by `parser::tests::loads_workflow_snapshot_from_directory_ignoring_mods_and_archive`). So `snapshot.items` never contains an entity whose file lives under `_archive/`.
   - `archived_done_count` is populated only from `load_archived_items()`, which reads `_archive/*.md` and `_archive/<dir>/index.md` exclusively (`src/parser/archive.rs`).
   - The two sources are therefore filesystem-disjoint: an entity is either in the active tree or in `_archive/`, never both. Summing the two counts cannot double-count a real entity. The id-collision case (two different files reusing the same `id` across active and archived) is out of scope — Spacedock's archive convention is "move the file", not "copy". If we ever need to defend against that, the right place is the parser, not the counts function.

3. **Module ownership.**
   - All code changes land in `src/app/overview.rs`. No parser change, no UI change.
   - Test additions land in `src/app/tests.rs` next to the existing archived-done test.
   - No frontmatter edits, no scaffolding edits.

## Test Strategy

- **AC-1 — Active-only done == 1.**
  New unit test in `src/app/tests.rs`: build a synthetic `WorkflowSnapshot` (use `App::from_snapshot` or a tempdir helper) with one active item at `status: "done"` and **no** `_archive/` directory. Assert `app.stage_counts()` finds the `done` stage with `items == 1`. With the current bug this returns `0` because `archived_done_count` is `Some(0)` after `refresh_archived_done_count()` runs against a missing archive — confirming the test fails before the fix and passes after.

- **AC-2 — Existing archived-only test continues to pass.**
  `stage_counts_include_archived_done_items_from_the_workflow_archive` in `src/app/tests.rs` already exercises archived-only against the real `docs/spacetop-dev` fixture. After the fix it must still report `done > 0`. Because the live workflow currently has no active `done` items, the observed count is unchanged from before the fix; the assertion `done.items > 0` continues to hold. Run it as-is — no edits.

- **AC-3 — Active + archived sum without double-count.**
  New unit test in `src/app/tests.rs`: build a tempdir workflow (reuse `write_workflow_with_archive` helper if present, or extend it) containing N active `done` items in the workflow root **and** M archived `done` items under `_archive/`, where the active and archived ids are distinct (e.g. active `100, 101`, archived `200, 201, 202`). Load with `App::load` and assert the `done` count equals `N + M`. This is the load-bearing regression for the inverse-of-032 bug.

## Verification Commands

```bash
cargo test app::tests::stage_counts_active_only_done_contributes_to_count -- --exact
cargo test app::tests::stage_counts_include_archived_done_items_from_the_workflow_archive -- --exact
cargo test app::tests::stage_counts_sum_active_and_archived_done_without_double_counting -- --exact
cargo test app::tests::stage_counts_reuse_cached_archived_done_count_after_archive_disappears -- --exact
make lint
```

(Test names in the commands above are placeholders for the implement stage; the implementer may pick clearer names as long as each AC is covered.)

## Stage Report: plan

- DONE: Plan names src/app/overview.rs::stage_counts as the fix site and explains the sum (not replace) of active terminal items + archived_done_count.
  Implementation Plan step 1 names `src/app/overview.rs::stage_counts` (lines ~310–333) and shows the `count.items += archived_done_count` change replacing the current overwrite at lines 324–331.
- DONE: Test strategy maps explicit cases to AC-1 (active-only done==1), AC-2 (archived-only continues to pass), and AC-3 (active + archived sum without double-count).
  Test Strategy section enumerates one test per AC, each tied to the named acceptance criterion and located in `src/app/tests.rs`.
- DONE: Plan flags any risk of double-count when an item could appear both active and archived, and states how to keep the two sources disjoint.
  Implementation Plan step 2 documents the filesystem disjointness: `load_workflow_dir` skips `_archive/` (covered by an existing parser test) and `load_archived_items` reads only under `_archive/`, so `snapshot.items` and `archived_done_count` never share an entity.

### Summary

This planning pass scopes the fix to a one-line change in `OverviewState::stage_counts` — replace the overwrite with a sum — and leaves the parser/UI untouched. Disjointness between active and archived sources is guaranteed by the existing parser boundary, so summing is safe. Three focused tests (AC-1 active-only, AC-2 archived-only existing test, AC-3 mixed sum) cover the regression surface; the existing cached-after-archive-disappears test continues to constrain the `Option<usize>` cache contract.
