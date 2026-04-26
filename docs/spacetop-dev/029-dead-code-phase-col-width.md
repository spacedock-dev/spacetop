---
id: "029"
title: "Dead code warning: phase_col_width is never used"
status: review
source: bug report
started: 2026-04-26T06:41:03Z
completed:
verdict:
score: 0.6
worktree: .worktrees/spacedock-ensign-029-dead-code-phase-col-width
issue:
pr:
---

`cargo build` emits a dead code warning after PR #17 was merged:

```
warning: function `phase_col_width` is never used
   --> src/ui/mod.rs:102:15
    |
102 | pub(crate) fn phase_col_width(items: &[&crate::domain::WorkItem]) -> usize {
    |               ^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `spacetop` (lib) generated 1 warning
```

## Root cause

During the PR #17 review, the `phase_col_width()` call in `build_task_list_items` was inlined (per Copilot's suggestion) to avoid a Vec allocation. The standalone `phase_col_width` function remained in the codebase but is now unreachable from any production code path. It is still used in a unit test (`phase_col_width_uniform_short_phases_clamped_to_4` etc.), but test usage does not suppress the `dead_code` lint on the function itself in lib context.

## Fix

Either:
1. Remove `phase_col_width` entirely and update the three unit tests that call it to use the inlined expression directly, **or**
2. Keep the function and mark it `#[cfg(test)]` since it is only used in tests — but this couples the production API to test visibility, which is a worse design.

Option 1 is preferred: the function is simple enough that the tests can call the inlined `max().clamp()` expression directly without the helper.

## Acceptance criteria

**AC-1 -- No dead_code warning on cargo build.**
`cargo build 2>&1 | grep "dead_code\|never used"` returns no output.
Verified by: CI / local build.

**AC-2 -- All existing tests still pass.**
The three phase_col_width snapshot tests are updated to not call the removed function; 174/174 (or current count) tests pass.

## Stage Report: design

- DONE: Problem statement confirms phase_col_width call sites — production code (none) vs test code (3 tests).
  Verified by grep: `phase_col_width` is defined at `src/ui/mod.rs:102` as `pub(crate) fn`; called only in test code at lines 1958, 1989, and 2009; no call sites exist outside the `#[cfg(test)]` module. All other occurrences are comments.
- DONE: Fix direction confirmed: remove function, inline expression in tests.
  Entity body already documents Option 1 as preferred (remove function, use `items.iter().map(|i| i.status.chars().count()).max().unwrap_or(4).clamp(4, 12)` directly in each of the 3 tests). No production code references the function, so deletion is safe.

### Summary

The `phase_col_width` function at `src/ui/mod.rs:102` has zero production call sites — its only call was inlined during PR #17 review. Three unit tests (lines 1958, 1989, 2009) still invoke it by name, but test usage does not suppress the `dead_code` lint. The fix direction is confirmed: delete the function and replace all three `super::phase_col_width(&items_ref)` calls with the inlined expression `items_ref.iter().map(|i| i.status.chars().count()).max().unwrap_or(4).clamp(4, 12)`.
