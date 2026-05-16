---
id: "040"
title: Worktree-aware diff orientation and 'o' open target
status: implement
source: captain
started: 2026-05-16T08:44:53Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-040-worktree-aware-diff-and-open
issue:
pr:
---

The preview-pane diff and the `o` (open in editor) action both need to treat the worktree copy as the active version of an entity when one exists.

Two related behaviors to align:

1. **Diff orientation in the preview body.** When an entity has a divergent worktree copy, the preview renders a unified diff between the main body and the worktree body. The worktree content should appear as the *added* side (`+` green). Verify the current call site in `src/ui/mod.rs` (around the `render_diff_lines(main, &item.body)` invocation) and confirm — or fix — the argument order so worktree content reads as additions and main content as removals.

2. **`o` opens the worktree copy when present.** Pressing `o` in the overview currently records `pending_open_file` from the item's path. When the entity has a `worktree_source` (i.e. there is a worktree copy on disk), `o` should open the worktree-resident markdown file in `$EDITOR` (nvim), not the main-branch copy. When no worktree copy exists, behavior is unchanged.

Both changes should be testable without a terminal backend: extend `src/ui/diff.rs` tests for orientation, and extend the `App` keymap / `pending_open_file` tests in `src/app.rs` to assert the chosen path when `worktree_source` is `Some(_)` vs `None`.

## Acceptance criteria

**AC-1 -- Preview diff shows worktree content as `+` and main content as `-`.**
Verified by: a unit test in `src/ui/diff.rs` (or the existing preview render tests in `src/ui/mod.rs`) that constructs a `WorkItem` with distinct `main_body` and `body`, renders the bottom preview, and asserts a line beginning with `+` carries text unique to the worktree body and a line beginning with `-` carries text unique to the main body.

**AC-2 -- Pressing `o` on an entity with a worktree copy queues the worktree-resident markdown path for $EDITOR.**
Verified by: a unit test against `App` (mirroring the existing `pending_open_file` tests) that sets `worktree_source = Some(<worktree path>)` on the selected item, sends the `o` key, and asserts `take_pending_open_file()` returns the worktree path — not the main-branch path. A second test with `worktree_source = None` asserts the main path is still returned.

**AC-3 -- `make lint` and `cargo test` pass.**
Verified by: `make lint` (clippy with `-D warnings`) and `cargo test` from the repo root, both green.

## Implementation Plan

### Investigation summary

- Diff helper signature: `pub fn render_diff_lines(old: &str, new: &str)` in `src/ui/diff.rs:11`. `old` becomes `-` (red), `new` becomes `+` (green).
- Call site: `src/ui/mod.rs:667` — `diff::render_diff_lines(main, &item.body)`, where `main = item.main_body.as_deref()`. So `main_body` is passed as `old` (rendered `-`) and the worktree-divergent `item.body` is passed as `new` (rendered `+`). **Argument order is already correct.** Only a regression-locking test is needed for AC-1.
- `o` keypath: `src/app/keys.rs:83` emits `OverviewKeyAction::OpenSelectedFile(item.path.clone())`. The action is consumed in `src/app.rs:429-431` and stored in `pending_open_file`. The keypath, not the consumer, is where the source-of-truth path is chosen — fix belongs in `keys.rs:83`.
- `WorkItem.worktree_source: Option<PathBuf>` (`src/domain/...:140`) already holds the worktree-resident markdown path when a worktree copy exists; otherwise `None`. No new struct field required.

### Step 1 — Add regression test for diff orientation (AC-1)

File: `src/ui/diff.rs`, inside `#[cfg(test)] mod tests`.

Add test fn:

```
#[test]
fn render_diff_lines_treats_new_as_plus_old_as_minus() {
    let old = "shared\nONLY_IN_MAIN\nshared2\n";
    let new = "shared\nONLY_IN_WORKTREE\nshared2\n";
    let lines = render_diff_lines(old, new);
    let texts: Vec<String> = lines.iter().map(line_text).collect();
    assert!(texts.iter().any(|t| t == "-ONLY_IN_MAIN"),
        "main-only content must render with '-' prefix, got: {texts:?}");
    assert!(texts.iter().any(|t| t == "+ONLY_IN_WORKTREE"),
        "worktree-only content must render with '+' prefix, got: {texts:?}");
}
```

This locks in the contract that the call site in `ui/mod.rs:667` relies on. No production code changes for AC-1.

### Step 2 — Route `o` to worktree path when present (AC-2)

File: `src/app/keys.rs:82-85`. Replace the `o` arm body so it prefers `worktree_source` over `path`:

```
KeyCode::Char('o') if state.preview_open() => match state.selected_item() {
    Some(item) => {
        let target = item.worktree_source.clone().unwrap_or_else(|| item.path.clone());
        OverviewKeyAction::OpenSelectedFile(target)
    }
    None => OverviewKeyAction::None,
},
```

No helper method needed on `WorkItem` or `App` — a single inline `clone().unwrap_or_else(...)` keeps the keymap layer responsible and avoids API surface growth.

### Step 3 — Add `o`-keypath tests (AC-2)

File: `src/app/keys.rs`, inside `#[cfg(test)] mod tests`.

The existing `single_session_with_item` fixture builds a `WorkItem` with `worktree_source: None`. Extend it (or add a sibling fixture `single_session_with_item_worktree(path, worktree_path)`) that takes both paths and assigns `worktree_source = Some(worktree_path)`.

Add two tests:

1. `o_with_worktree_source_opens_worktree_path` — sets `worktree_source = Some(PathBuf::from("/tmp/wt/task-001.md"))`, sends `o` with preview open, asserts `OpenSelectedFile(worktree_path)` (not the main path).
2. `o_without_worktree_source_opens_main_path` — same as the existing `o_with_preview_open_emits_open_file_intent` but stated explicitly as the `None` branch; can also be satisfied by leaving the current test in place and adding only test (1).

### Step 4 — Verification

Run from repo root inside the dispatched worktree (`.worktrees/spacedock-ensign-040-worktree-aware-diff-and-open`):

- `make lint` — must pass with no clippy warnings (`-D warnings`).
- `cargo test` — all unit + integration tests green. In particular:
  - `ui::diff::tests::render_diff_lines_treats_new_as_plus_old_as_minus` (AC-1)
  - `app::keys::tests::o_with_worktree_source_opens_worktree_path` (AC-2)
  - existing `o_with_preview_open_emits_open_file_intent` (regression / AC-2 None branch)

### Module ownership notes

- All edits are confined to `src/ui/diff.rs` (test only) and `src/app/keys.rs` (one-line production change + tests).
- No changes to `src/ui/mod.rs`, `src/app.rs`, or `src/domain/`. The existing preview rendering and `pending_open_file` consumer at `src/app.rs:429-431` and `src/lib.rs:217` already do the right thing once the keypath supplies the worktree path.
- Keep the test-only fixture changes in `keys.rs` self-contained; do not modify `WorkItem` or `OverviewSession`.

## Stage Report: plan

- DONE: Identify the exact call site of `render_diff_lines` in `src/ui/mod.rs` and name the file:line; specify whether argument order needs to change for worktree content to render as `+`, or whether it is already correct and only tests need to be added to lock it in.
  `src/ui/mod.rs:667` — `diff::render_diff_lines(main, &item.body)` with `render_diff_lines(old, new)` signature at `src/ui/diff.rs:11`. Argument order is already correct (main → `-`, worktree body → `+`); plan adds a regression test only, no production change for AC-1.
- DONE: Identify where `pending_open_file` is assigned in `src/app.rs` and describe the change to route to the worktree-resident path when `worktree_source` is `Some(_)`. Name any helper needed on `WorkItem` or `App`.
  Assigned at `src/app.rs:430` from `OverviewKeyAction::OpenSelectedFile(path)`. The path choice lives upstream at `src/app/keys.rs:83`; plan modifies that arm to prefer `item.worktree_source.clone().unwrap_or_else(|| item.path.clone())`. No new helper on `WorkItem` or `App` is required.
- DONE: Name the specific tests to add (file + test fn name) and the verification commands (`make lint`, `cargo test`) that prove AC-1, AC-2, AC-3.
  AC-1: `src/ui/diff.rs::tests::render_diff_lines_treats_new_as_plus_old_as_minus`. AC-2: `src/app/keys.rs::tests::o_with_worktree_source_opens_worktree_path` plus the existing `o_with_preview_open_emits_open_file_intent` for the `None` branch. AC-3: `make lint` and `cargo test` from repo root.

### Summary

Plan keeps production changes minimal: a single line in `src/app/keys.rs:83` to prefer `worktree_source` over `path` when emitting `OpenSelectedFile`. The diff orientation in `src/ui/mod.rs:667` is already correct given the `render_diff_lines(old, new)` contract — it only needs a regression test in `src/ui/diff.rs`. No new API surface on `WorkItem` or `App`; the consumer at `src/app.rs:430` and `src/lib.rs:217` is untouched.

## Stage Report: implement

- DONE: AC-1 regression test passes: a test in `src/ui/diff.rs` asserts `render_diff_lines(old, new)` renders `new` content with `+` and `old` content with `-` (lock in current orientation).
  `src/ui/diff.rs::tests::render_diff_lines_treats_new_as_plus_old_as_minus` — passes (`cargo test --lib -- render_diff_lines_treats_new_as_plus_old_as_minus`).
- DONE: AC-2 production change + tests: pressing `o` on an item whose `worktree_source` is `Some(_)` queues the worktree-resident path; with `None` it queues the main path. Cover both branches with tests.
  Production change at `src/app/keys.rs:82-90` (prefer `worktree_source` via `clone().unwrap_or_else(|| item.path.clone())`). Tests `o_with_worktree_source_opens_worktree_path` and `o_without_worktree_source_opens_main_path` in `src/app/keys.rs` — both pass.
- DONE: AC-3 lint and tests green: `make lint` (or equivalent) exits 0, and `cargo test` from the worktree root passes.
  `make lint` exits 0 (clippy clean with `-D warnings`). All four targeted tests pass. Note: one pre-existing failure (`ui::graph::tests::narrow_tier_renders_compact_textual_summary`) reproduces on the dispatch commit before my changes — unrelated to this task; flagged but not addressed.

### Summary

Implemented per plan: single production change in `src/app/keys.rs` to route `o` to `worktree_source` when present, plus a regression test in `src/ui/diff.rs` that locks in the diff orientation contract used at `src/ui/mod.rs:667`. Extended the `keys.rs` test fixture with a `single_session_with_item_worktree` helper so both worktree/None branches are covered. Pre-existing `narrow_tier_renders_compact_textual_summary` failure was confirmed present on the dispatch commit and is out of scope.

## Stage Report: review

- DONE: AC-1 regression test present and passing. `src/ui/diff.rs:75-86` defines `render_diff_lines_treats_new_as_plus_old_as_minus`, which asserts `-ONLY_IN_MAIN` and `+ONLY_IN_WORKTREE` are emitted. `cargo test --lib -- render_diff_lines_treats_new_as_plus_old_as_minus` → `1 passed; 0 failed`. The test correctly locks the contract used at `src/ui/mod.rs:667` (`diff::render_diff_lines(main, &item.body)`).
- DONE: AC-2 production change correct and both branches tested. `src/app/keys.rs:83-89` reads `item.worktree_source.clone().unwrap_or_else(|| item.path.clone())` — prefers `Some(_)` and falls back to `path` on `None`. Tests `o_with_worktree_source_opens_worktree_path` (`src/app/keys.rs:185-208`) and `o_without_worktree_source_opens_main_path` (`src/app/keys.rs:212-223`) both pass; pre-existing `o_with_preview_open_emits_open_file_intent` still passes as a regression check on the `None` branch.
- DONE: AC-3 lint and tests green. `make lint` → `Finished dev profile … target(s) in 1.00s` with no warnings (clippy `-D warnings`). `cargo test` → `228 passed; 1 failed`. The single failure is `ui::graph::tests::narrow_tier_renders_compact_textual_summary` at `src/ui/graph.rs:861` (`missing narrow arrow`). Confirmed pre-existing by checking out `src/` at dispatch commit `3078e40` and reproducing the same panic — unrelated to this task's diff (`src/app/keys.rs` + `src/ui/diff.rs` only).

Recommendation: PASSED — ready for captain approval and merge.
