---
id: 009
title: Polish TUI (help popup, center alignment, colors) and clean up pre-existing test/lint drift
status: implement
source: captain feedback after 008 ship
started: 2026-04-25T00:31:23Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-polish-tui-and-cleanup-drift
issue:
pr:
---

Combined polish + cleanup task. Captain asked to skip design/plan and implement directly.

### Feedback Cycles

- **cycle 1 (2026-04-25, review → implement):** `cargo test` fails 1 test on the worktree branch — `ui::tests::preview_status_value_is_stage_colored` panics at `src/ui/mod.rs:617` with "expected status value in preview to use stage color Magenta". Self-authored AC-3 test. Tally was 84 passed / 1 failed (implementer claimed 85/0). Fix: either correct the column-walk in the test or ensure the cell immediately following the literal `"status: "` carries the stage `fg`. No need to re-verify other ACs.

## Scope

### UX additions

1. **Help popup** — overlay widget bound to `?` (and maybe `h`) showing the keymap (Up/Down/Home/End/Enter/q/Esc/`a`/etc.). Closes on `?`/`Esc`/any key. Renders centered over the overview, dimmed background. Visible from both `Picker` and `Overview` modes.
2. **Center alignment** — when the terminal is wider than the workflow content, horizontally center the workflow ribbon, task list, and preview pane (or the whole dashboard column) so it doesn't hug the left edge on wide terminals. Pick a sensible max content width (e.g. 120 cols) and center the column inside the available space.
3. **Colorful dashboard** — apply a thoughtful palette: stage status colors (e.g. design = blue, plan = cyan, implement = yellow, review = magenta, done = green), feedback arrow accent in red/orange, selected row highlight clearer, gate/worktree glyphs in distinct colors, archived rows muted. Don't go circus — keep it readable.

### Cleanup

4. Fix the pre-existing `clippy::unnecessary_lazy_evaluations` lint at `src/parser.rs` so `cargo clippy --all-targets -- -D warnings` is clean.
5. Fix the pre-existing fixture-drift test failures that have been surviving every PR's review:
   - `app::tests::loads_real_workflow_state_and_derives_stage_counts`
   - `ui::tests::renders_real_workflow_summary_task_list_and_preview`
   - `ui::graph::tests::header_row_contains_scope_label_and_workflow_path`
   These tests hard-code expectations against specific `docs/spacetop-dev/` workflow content that keeps changing as we ship. Fix by deriving expected values from the loaded snapshot (same trick task 003 used for the original status-coupling failures) instead of hard-coding stage names/counts/titles.

## Acceptance criteria

**AC-1 -- `?` opens a centered help popup that lists the keymap; `?`/`Esc` closes it; popup renders in both Picker and Overview modes.**
Verified by: render test asserts popup buffer contains the keymap labels; toggle test asserts state flips on `?`.

**AC-2 -- On terminals wider than the workflow content, the dashboard column is horizontally centered (not left-hugging).**
Verified by: render test against a wide `TestBackend` (e.g. 200×40) asserts left margin is non-zero and roughly equals right margin.

**AC-3 -- Stage status is rendered with distinct colors across the ribbon, list, and preview.**
Verified by: render test inspects style spans on a known stage cell (e.g. `done` cell uses green foreground or distinct style).

**AC-4 -- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` all pass on the worktree branch with no surviving pre-existing exceptions.**
Verified by: command output cited in the implement stage report.

**AC-5 -- The three named fixture-drift tests pass without hard-coding workflow-content expectations.**
Verified by: tests reread the snapshot for their expectations; grep shows the test bodies no longer contain hard-coded stage names like `implement: 1` or specific archived task titles.
