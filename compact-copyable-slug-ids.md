---
title: Keep long slug IDs compact and copyable
status: implement
source: "UI feedback from task-list screenshot on 2026-07-28"
kind: feature
risk: medium
milestone: v1-maintenance
proof: Ratatui task-list width tests plus mouse double-click copy tests
started: 2026-07-28T03:26:42Z
completed:
verdict:
score: 0.78
worktree: .worktrees/spacedock-ensign-compact-copyable-slug-ids
issue:
pr:
id: 074
---

Long slug-style IDs currently expand the task-list ID column without an upper bound, leaving too little room for the title. Keep the visible slug ID within a bounded, responsive width so task titles retain useful space.

The full slug must remain easy to retrieve: double-clicking the ID cell should copy the complete, untruncated slug even when the visible value is ellipsized. This must work with Spacetop mouse capture enabled.

## Acceptance criteria

- **AC-1:** In an `id-style: slug` workflow, long IDs have a bounded visible width and cannot compress the title below the intended minimum title space.
- **AC-2:** Long visible IDs are ellipsized while short slug IDs and numeric or sequential IDs remain readable without unnecessary truncation.
- **AC-3:** Double-clicking inside an entity ID cell copies the exact complete slug, not the displayed truncated value.
- **AC-4:** Double-clicking outside the ID cell does not copy an ID, and existing single-click row selection and mouse scrolling continue to work.
- **AC-5:** The TUI provides brief, non-disruptive confirmation after copying an ID.
- **AC-6:** Ratatui rendering tests cover long and short IDs at narrow and wide terminal sizes; app/input tests cover double-click detection, hit testing, and the exact copied value.

## Implementation context

`crates/spacetop/src/ui/list.rs` currently sizes the ID column from the longest visible ID with no upper clamp. `crates/spacetop/src/lib.rs` enables Crossterm mouse capture, so the copy interaction must preserve existing mouse navigation.

## Implementation plan

1. In `crates/spacetop/src/ui/list.rs`, replace the unbounded ID width with pure helpers and pinned constants: `ID_COL_MIN = 4`, `ID_COL_MAX = 20`, and `TITLE_COL_MIN = 16`. Compute the fixed row cost as the 2-cell gutter, phase width, 1-cell phase/ID gap, 2-cell ID/title gap, and the two 2-cell activity/worktree marker columns; cap the natural ID width by both 20 and the remaining pane budget after reserving 16 title cells.
2. Preserve the 4-cell ID floor when the pane is too small to satisfy every minimum. Short IDs remain right-aligned and unmodified; an over-width ID keeps its first `width - 1` characters and ends with `…`. Thus `074` remains ` 074`, ordinary short slugs remain complete, and only values that exceed the responsive width are ellipsized.
3. Have the list renderer record an `id_column_rect` render fact on `OverviewState`, alongside `list_rows_rect` and `list_offset`. Its x-coordinate is the drawn list origin plus the gutter, phase column, and phase/ID gap; reset it when no entity ID cells are drawn. Mouse mapping combines this rect with the rendered list offset and rejects synthetic broken rows.
4. In `crates/spacetop/src/app/mouse.rs`, add a 500 ms double-click detector over two left-button `Down` events. The first press must hit a real entity ID cell and records workflow, exact entity ID, terminal cell, and time; the matching second press at the same cell queues that stored full ID and clears the tracker.
5. Anchor the gesture to the first rendered ID hit so the existing first-click action can still select the row and open the preview immediately, even if that redraw narrows the list before the second press. A nonmatching press, wheel, drag, workflow change, or timeout cancels/restarts the candidate; button `Up` does not, because it occurs between the two presses.
6. Extend `OverviewKeyAction`/`App` with a pending copy intent and a clock-injected `handle_mouse_at` test seam. Keep the copied value equal to `Entity.id`, never the ellipsized span, and preserve the existing selection, preview, divider-drag, picker, help-popup, and wheel paths.
7. In `crates/spacetop/src/lib.rs`, drain the intent at the terminal boundary and emit OSC 52 (`ESC ] 52 ; c ; <base64> BEL`) through the active Ratatui `CrosstermBackend`, then flush it without disabling mouse capture. Add `base64 = "0.22"` as a direct `spacetop` dependency: the crate is already locked transitively, and using its standard encoder avoids a handwritten protocol primitive.
8. Store a transient copy outcome on `App`; `crates/spacetop/src/ui/footer.rs` prepends `✓ ID copied` in green or `⚠ ID copy failed` in red and expires it after two seconds via an injected-time seam. The compact label avoids putting the long slug into the one-line footer.
9. Add Ratatui tests in `crates/spacetop/src/ui/tests/task_list.rs` for long and short IDs at 80- and 160-column terminals, the 20-cell cap, responsive shrinkage with at least 16 title cells when geometry permits, right-aligned numeric IDs, exact ellipsis text, and ID-rect pixels. Extend `crates/spacetop/src/app/mouse.rs` tests for full-value copy, outside-cell/timeout rejection, first-click reflow, scrolled rows, unchanged single-click selection, and unchanged wheel behavior.
10. Add `lib.rs` `Vec<u8>` tests that pin the exact OSC 52 bytes and prove the sequence can be written between Crossterm mouse-enable/disable commands. Add footer/help tests for feedback success, failure, expiry, and the new help line.
11. Update the README Mouse section and `ui/help.rs` with “Double-click ID: copy full ID” plus the OSC 52 terminal-support note. No parser, core/index, workflow schema, session persistence, or workflow-markdown write path changes are needed.

Proof commands:

```bash
cargo fmt --check
cargo test -p spacetop ui::tests::task_list
cargo test -p spacetop app::mouse::tests
cargo test -p spacetop osc52
cargo test
make lint
```

The plan-stage protocol spike used the current `ratatui 0.30`, `crossterm 0.28`, and `base64 0.22` APIs with a `CrosstermBackend` backed by `Vec<u8>`; it observed the exact OSC 52 payload for `compact-copyable-slug-ids` between the mouse-capture enable and disable sequences. Existing baselines also pass: 31 task-list tests and 11 overview-mouse tests.

## Stage Report: plan

- DONE: Pin a responsive slug-column width and ellipsis policy that preserves useful title space without regressing short or numeric IDs.
  The plan fixes a 4-cell floor, 20-cell cap, 16-cell title reserve, pane-budget shrinkage, right-aligned unchanged short IDs, and trailing ellipsis for long IDs.
- DONE: Choose and de-risk a full-ID double-click copy mechanism compatible with mouse capture, including exact hit testing, feedback, and unchanged single-click/scroll behavior.
  A protocol spike proved OSC 52 writes through Ratatui's Crossterm backend while capture stays enabled; the 500 ms first-hit tracker, render facts, and transient footer pill cover input and feedback.
- DONE: Name the owning modules, dependency decision, lowest-layer rendering/input tests, documentation impact, and exact verification commands.
  The eleven-step plan assigns UI, app/mouse, terminal, Cargo, help/README, targeted tests, full tests, formatting, and lint without touching core parsing or workflow state.

### Summary

The implementation path keeps long IDs within a responsive 4–20-cell column while reserving 16 cells for titles whenever the pane can satisfy the minimums. Full IDs are copied through OSC 52 after a render-fact-backed double-click, with existing mouse navigation preserved and a brief footer confirmation.
