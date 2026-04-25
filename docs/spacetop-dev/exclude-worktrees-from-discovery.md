---
id: "012"
title: Exclude .worktrees/ directories from workflow auto-discovery
status: review
source: feature request — first-officer session 2026-04-25
started: 2026-04-25T09:57:56Z
completed:
verdict:
score: 0.7
worktree: .worktrees/spacedock-ensign-exclude-worktrees-from-discovery
issue:
pr:
mod-block: merge:pr-merge
---

When Spacetop auto-discovers workflow directories (via `--discover` or directory scanning), it may walk into `.worktrees/` subdirectories and treat each worktree clone as an independent workflow. Each active Spacedock task checked out into a git worktree contains a full copy of the project including `docs/` — which means the same workflow directory appears N+1 times (once on main, once per active worktree). This causes duplicated workflow entries in the picker and graph view, and confuses the overview state.

## Context

Spacedock first-officer agents check out each `implement`-stage task into a dedicated worktree under `.worktrees/{worker-slug}/`. These worktrees are project-root replicas — they contain the same `docs/spacetop-dev/`, `docs/spacetop-ui/`, etc. as main. Spacetop's discovery currently has no exclusion for these paths.

Worktrees are ephemeral and not meant to be browsed as top-level workflow contexts.

## Proposed fix

In `src/discovery.rs` (or wherever `--discover` / directory scanning is implemented):
- Skip any path component that is `.worktrees` when recursing into workflow directories.
- Concretely: after resolving candidate dirs, filter out any path whose canonicalized form has `.worktrees` as an ancestor component.

Also consider: display a hint in the picker when a path is inside `.worktrees/`, or add a warning to the graph view header if the loaded workflow path is inside a worktree.

## Acceptance criteria

**AC-1 — `--discover` does not return paths under `.worktrees/`.**
Verified by: unit test or integration test in `tests/` that creates a fake `.worktrees/some-task/docs/workflow/` directory structure and asserts `--discover` output does not include it.

**AC-2 — A workflow opened at a project root that has active worktrees shows only one entry per distinct logical workflow (not N+1 for each worktree clone).**
Verified by: `cargo test` with a fixture that has both a real workflow dir and a `.worktrees/` sibling — picker item count equals real-workflow count.

**AC-3 — The filter does not break discovery when `.worktrees/` is absent.**
Verified by: existing discovery tests continue to pass with no modification.

## Implementation Plan

### Current state

`.worktrees` is already listed in `PRUNED_DIR_NAMES` in `src/discovery.rs` (line 31). The `is_pruned` function uses this constant to skip entire subtrees during `WalkDir` traversal. The discovery logic is correct. What is missing are explicit test cases proving AC-1 and AC-2 coverage.

No changes to production source code are required. The implementation work is entirely in tests.

### Step 1 — Add unit tests for `.worktrees` exclusion in `src/discovery.rs`

**File:** `src/discovery.rs`, inside the existing `#[cfg(test)] mod tests` block.

**Tests to add:**

**Test A — AC-1: `discover_workflows` returns nothing under `.worktrees/`**
```
fn worktrees_subdir_is_excluded_from_discovery()
```
- Fixture: `tempdir` with `docs/real/README.md` (valid workflow) and `.worktrees/some-task/docs/real/README.md` (same workflow inside worktree clone).
- Assertion: `discover_workflows(root)` returns exactly 1 workflow and its root does not contain `.worktrees` as a path component.

**Test B — AC-2: picker item count equals real-workflow count (not N+1)**
```
fn worktrees_clone_does_not_inflate_workflow_count()
```
- Fixture: `tempdir` with `docs/alpha/README.md`, `docs/beta/README.md` (2 real workflows), `.worktrees/task-1/docs/alpha/README.md`, `.worktrees/task-1/docs/beta/README.md` (worktree clone with same 2 workflows).
- Assertion: `discover_workflows(root).len() == 2`.

### Step 2 — Add integration test for `.worktrees` exclusion in `tests/discovery_bypass.rs`

**File:** `tests/discovery_bypass.rs`, new test function.

**Test C — AC-1+AC-2 via `decide_app`:**
```
fn worktrees_excluded_from_decide_app_discovery()
```
- Fixture: `tempdir` with `.git/`, `docs/real/README.md` (valid full workflow using `write_workflow`), `.worktrees/some-task/docs/real/README.md` (same).
- Call `decide_app(&cli_with(None), root)`.
- Assertion: outcome is `DecideOutcome::Overview(app)` where `app.as_session().unwrap().len() == 1`, confirming no duplication from the worktree clone.

### Step 3 — Verify AC-3 (no regression)

Run `cargo test` and confirm all pre-existing tests in `src/discovery.rs` and `tests/discovery_bypass.rs` still pass. No code changes needed; this is a verification-only step.

**Command:** `cargo test 2>&1 | tail -20`

### File ownership

| File | Change |
|------|--------|
| `src/discovery.rs` | Add 2 unit tests inside existing `#[cfg(test)]` block |
| `tests/discovery_bypass.rs` | Add 1 integration test using existing `write_workflow` helper |

### Evidence of completion

- `cargo test` output showing all tests pass (including the 3 new tests by name).
- No changes to `PRUNED_DIR_NAMES`, `is_pruned`, or `discover_workflows` — production code is already correct.

## Stage Report: plan

- DONE: Step-by-step implementation plan naming exact files, functions, and test locations.
  Plan above covers all 3 ACs: 2 unit tests in `src/discovery.rs`, 1 integration test in `tests/discovery_bypass.rs`. Production code (`PRUNED_DIR_NAMES` already includes `.worktrees`) requires no changes.
- DONE: Test strategy: at least one proposed test fixture and assertion for each AC.
  AC-1: `worktrees_subdir_is_excluded_from_discovery` — fixture has `.worktrees/…/README.md`, asserts 1 result with no `.worktrees` path component. AC-2: `worktrees_clone_does_not_inflate_workflow_count` — fixture has 2 real workflows + 2 in a worktree clone, asserts `len() == 2`. AC-3: existing tests pass without modification.

### Summary

The production fix (`.worktrees` in `PRUNED_DIR_NAMES`) is already in place in `src/discovery.rs`. The plan calls for adding three new tests — two unit tests in the `discovery` module and one integration test in `tests/discovery_bypass.rs` — that directly exercise each acceptance criterion using `tempfile` fixtures. No logic changes are needed; the work is entirely test coverage.

## Stage Report: implement

- DONE: All 3 new tests pass: worktrees_subdir_is_excluded_from_discovery, worktrees_clone_does_not_inflate_workflow_count (unit), worktrees_excluded_from_decide_app_discovery (integration).
  Confirmed by `cargo test worktrees`: 3 passed, 0 failed (commit 452193e).
- DONE: cargo test exits 0 with no regressions.
  112 lib tests + 5 integration tests all pass; 0 failed.

### Summary

Added two unit tests to `src/discovery.rs` and one integration test to `tests/discovery_bypass.rs` covering AC-1, AC-2, and AC-3. No production code was modified — `PRUNED_DIR_NAMES` already contained `.worktrees`. All 117 tests pass with `cargo test` exit 0.

## Stage Report: review

- DONE: Verify all 3 ACs have test evidence from the implement stage report.
  Implement report lists: `worktrees_subdir_is_excluded_from_discovery` (AC-1), `worktrees_clone_does_not_inflate_workflow_count` (AC-2), `worktrees_excluded_from_decide_app_discovery` (AC-1+AC-2 via `decide_app`). AC-3 covered by full `cargo test` exit 0 with 117 tests passing. All 3 ACs accounted for.
- DONE: Check the diff for correctness and that no production code was accidentally changed.
  `git diff main...HEAD --name-only` shows only 3 files changed: `docs/spacetop-dev/exclude-worktrees-from-discovery.md` (entity), `src/discovery.rs` (tests only, inside `#[cfg(test)]` block), `tests/discovery_bypass.rs` (new test function only). `PRUNED_DIR_NAMES` and all production functions are unmodified. Diff is correct.

### Summary

All 3 acceptance criteria are covered by named test evidence in the implement report. The diff is scoped entirely to test code and the entity file — no production logic was touched, which is correct since `.worktrees` was already in `PRUNED_DIR_NAMES`. Implementation is approved.
