---
id: "029"
title: "Dead code warning: phase_col_width is never used"
status: plan
source: bug report
started: 2026-04-26T06:41:03Z
completed:
verdict:
score: 0.6
worktree:
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
