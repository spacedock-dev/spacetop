---
title: Keep long slug IDs compact and copyable
status: verify
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
pr: "#78"
id: 074
mod-block: merge:pr-merge
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

### Feedback Cycles

- Cycle 1: REJECTED — verify; surface 1 blocking Unicode display-width defect vs estimate not declared (n/a%); AC unchanged

## Stage Report: plan

- DONE: Pin a responsive slug-column width and ellipsis policy that preserves useful title space without regressing short or numeric IDs.
  The plan fixes a 4-cell floor, 20-cell cap, 16-cell title reserve, pane-budget shrinkage, right-aligned unchanged short IDs, and trailing ellipsis for long IDs.
- DONE: Choose and de-risk a full-ID double-click copy mechanism compatible with mouse capture, including exact hit testing, feedback, and unchanged single-click/scroll behavior.
  A protocol spike proved OSC 52 writes through Ratatui's Crossterm backend while capture stays enabled; the 500 ms first-hit tracker, render facts, and transient footer pill cover input and feedback.
- DONE: Name the owning modules, dependency decision, lowest-layer rendering/input tests, documentation impact, and exact verification commands.
  The eleven-step plan assigns UI, app/mouse, terminal, Cargo, help/README, targeted tests, full tests, formatting, and lint without touching core parsing or workflow state.

### Summary

The implementation path keeps long IDs within a responsive 4–20-cell column while reserving 16 cells for titles whenever the pane can satisfy the minimums. Full IDs are copied through OSC 52 after a render-fact-backed double-click, with existing mouse navigation preserved and a brief footer confirmation.

## Stage Report: implement

- DONE: Implement the planned responsive 4–20-cell ID column, 16-cell title reserve, ellipsis behavior, and render facts needed for exact ID-cell hit testing.
  Commit `356411d`; the 34 task-list tests fail if 80/160-column panes stop yielding 9/20-cell ID widths, numeric IDs lose right alignment, ellipsis text changes, title reserve shrinks, or the recorded ID rect drifts from pixels.
- DONE: Implement 500 ms double-click copying of the full untruncated ID through OSC 52 with transient confirmation, while preserving single-click selection, scrolling, and outside-cell behavior.
  The 15 mouse tests fail if reflow loses the first-hit anchor, timeout/outside/broken rows copy, scroll offsets map incorrectly, wheel fails to cancel, or existing selection/scroll behavior changes; OSC 52 and footer tests pin exact bytes, capture ordering, outcomes, and expiry.
- DONE: Add focused Ratatui/input/terminal tests and user-facing documentation, then pass formatting, targeted tests, full cargo test, git diff checks, and make lint.
  `cargo fmt --all -- --check`, all targeted commands, final `cargo test` (383 Spacetop library tests, 188 core tests, integration and guardrail suites), `git diff --check`, and `make lint` passed; README and help document full-ID copying and OSC 52 support.

### Summary

Long IDs now use a responsive 4–20-cell column with trailing ellipsis and a 16-cell title reserve whenever geometry permits. Double-clicking a rendered ID cell queues the full underlying ID for OSC 52 output without disabling mouse capture, then shows a two-second success or failure pill while all existing mouse interactions remain intact.

## Stage Report: verify

- FAILED: Independently challenge all six acceptance criteria with adversarial narrow/wide layout, exact full-ID copy, timeout/outside-cell, reflow, scrolling, and mouse-capture cases.
  Verdict: REJECTED. ASCII paths, a 30-column minimum-conflict pane, reflow, scroll offsets, timeout, outside/broken rows, capture ordering, feedback, and expiry behaved as intended, but wide Unicode slug cells fail AC-1 through AC-3.
- FAILED: AC-1: In an `id-style: slug` workflow, long IDs have a bounded visible width and cannot compress the title below the intended minimum title space.
  `crates/spacetop/src/ui/list.rs:143-169` measures `chars()` instead of terminal-cell width; an exercised 10-character `資料資料資料資料資料` ID occupied 20 cells and moved the title from expected x=25 to x=35.
- FAILED: AC-2: Long visible IDs are ellipsized while short slug IDs and numeric or sequential IDs remain readable without unnecessary truncation.
  The same wide ID was treated as width 10 and left untruncated despite consuming 20 cells; ASCII long/short/numeric cases passed.
- FAILED: AC-3: Double-clicking inside an entity ID cell copies the exact complete slug, not the displayed truncated value.
  An exercised double-click at x=`id_rect.x + 18`, visibly within the 20-cell wide ID, produced no copy because `id_column_rect` recorded only 10 cells.
- DONE: AC-4: Double-clicking outside the ID cell does not copy an ID, and existing single-click row selection and mouse scrolling continue to work.
  The 15 mouse tests reject outside, timeout, wheel, and broken-row copies while pinning single-click selection, reflow anchoring, scroll offsets, picker behavior, and wheel behavior.
- DONE: AC-5: The TUI provides brief, non-disruptive confirmation after copying an ID.
  Footer tests pin green success, red failure, and expiry exactly at two seconds; terminal code maps OSC 52 write/flush outcomes without disrupting mouse capture.
- FAILED: AC-6: Ratatui rendering tests cover long and short IDs at narrow and wide terminal sizes; app/input tests cover double-click detection, hit testing, and the exact copied value.
  The committed tests cover the ASCII cases but omit terminal-cell-width IDs; temporary adversarial Ratatui and mouse probes reproduced the title/rect drift and uncopyable rendered tail.
- DONE: Review commit 356411d for correct UI/app/terminal ownership, OSC 52 safety and portability, dependency justification, read-only boundaries, and regression risk.
  UI render facts, app intent, and terminal emission are correctly separated; base64 makes OSC 52 payloads injection-safe, docs disclose terminal support, the direct dependency is justified, and no workflow-write path changed.
- DONE: Run focused rendering/input/protocol tests plus full cargo test, formatting, diff checks, and make lint; issue an explicit PASSED or REJECTED verdict with evidence.
  The 34 task-list, 15 mouse, and 2 OSC 52 tests passed; `cargo fmt --all -- --check`, full `cargo test`, `git diff --check`, clean worktree checks, and `make lint` passed, but the verdict is REJECTED on the reproduced Unicode regression.

### Summary

Commit `356411d` is healthy on its tested ASCII path and keeps the read-only and terminal ownership boundaries intact, but it confuses Unicode scalar count with terminal-cell width. Bounce back to size, truncate, and hit-test IDs by display-cell width, then add wide/combining-character rendering and rendered-tail double-click tests before re-verification.

## Stage Report: implement (cycle 2)

- DONE: Measure, budget, truncate, pad, and record slug ID rectangles by terminal display-cell width so wide and combining Unicode cannot exceed the responsive cap or shift the title reserve.
  Commit `f9bee09`; `wide_and_combining_ids_use_terminal_cell_width` fails if the CJK or combining IDs exceed the 9-cell narrow budget, lose cell-aware padding or ellipsis, or move the title beyond the reserved start.
- DONE: Add wide-Unicode and combining-character rendering tests plus a rendered-tail double-click test that copies the complete underlying ID, while preserving existing ASCII behavior.
  The 35 task-list and 16 mouse tests fail if wide or combining glyph geometry drifts, the visible CJK tail escapes `id_column_rect`, full-ID copying truncates, or the accepted ASCII, reflow, timeout, outside-cell, or scroll behavior regresses.
- DONE: Run focused task-list and mouse tests, full cargo test, formatting, diff checks, and make lint; commit the corrected worktree branch and report exact evidence.
  `cargo fmt --all -- --check`, `git diff --check`, focused suites, final `cargo test` (385 Spacetop library tests, 188 core tests, integration and guardrail suites), and `make lint` passed at `f9bee09`.

### Summary

The correction replaces scalar counting with `unicode-width` display-cell measurement for ID budgeting, truncation, padding, and hit-test geometry. CJK and combining IDs now stay inside the responsive column and preserve the title reserve, while double-clicking the rendered Unicode tail copies the complete underlying ID and all accepted ASCII behavior remains green.

## Stage Report: verify (cycle 2)

- DONE: Independently challenge all six acceptance criteria with adversarial narrow/wide layout, exact full-ID copy, timeout/outside-cell, reflow, scrolling, and mouse-capture cases.
  Verdict: PASSED. Wide and combining IDs, a 24-cell CJK slug truncated into a 20-cell column, the 30-column minimum-conflict pane, reflow, scroll offsets, timeout, outside/broken rows, capture ordering, feedback, and expiry all behaved as required.
- DONE: AC-1: In an `id-style: slug` workflow, long IDs have a bounded visible width and cannot compress the title below the intended minimum title space.
  `f9bee09` budgets natural width with `UnicodeWidthStr`; committed CJK/combining rendering assertions pin the 9-cell narrow rectangle and exact title start, while ASCII 80/160-column tests retain the 16-cell reserve and 20-cell cap.
- DONE: AC-2: Long visible IDs are ellipsized while short slug IDs and numeric or sequential IDs remain readable without unnecessary truncation.
  Display-cell-aware prefix selection and padding put the ellipsis in the final budgeted cell; long CJK/combining and ASCII cases truncate correctly, while short combining, short ASCII, and `074` remain complete and aligned.
- DONE: AC-3: Double-clicking inside an entity ID cell copies the exact complete slug, not the displayed truncated value.
  The committed Unicode-tail test copies the full underlying ID, and an independent 24-cell CJK probe double-clicked the rendered ellipsis cell in a 20-cell rectangle and returned the exact untruncated slug.
- DONE: AC-4: Double-clicking outside the ID cell does not copy an ID, and existing single-click row selection and mouse scrolling continue to work.
  The 16 mouse tests reject outside, timeout, wheel, and broken-row copies while pinning single-click selection, reflow anchoring, scroll offsets, picker behavior, and wheel behavior.
- DONE: AC-5: The TUI provides brief, non-disruptive confirmation after copying an ID.
  Footer tests pin green success, red failure, and expiry exactly at two seconds; OSC 52 tests pin exact bytes and emission between mouse-enable and mouse-disable sequences.
- DONE: AC-6: Ratatui rendering tests cover long and short IDs at narrow and wide terminal sizes; app/input tests cover double-click detection, hit testing, and the exact copied value.
  The 35 task-list and 16 mouse tests cover responsive ASCII, wide and combining display width, hit rectangles, full-value copy, reflow, scrolling, timeout, outside cells, and broken rows; the independent truncated-CJK composition probe also passed.
- DONE: Review commit 356411d for correct UI/app/terminal ownership, OSC 52 safety and portability, dependency justification, read-only boundaries, and regression risk.
  Correction `f9bee09` stays inside UI measurement/tests plus a direct already-locked `unicode-width` dependency; UI render facts, app intent, terminal OSC 52 emission, and read-only/no-write boundaries remain separated and guardrail-tested.
- DONE: Run focused rendering/input/protocol tests plus full cargo test, formatting, diff checks, and make lint; issue an explicit PASSED or REJECTED verdict with evidence.
  All 35 task-list, 16 mouse, and 2 OSC 52 tests passed; `cargo fmt --all -- --check`, full `cargo test` (385 Spacetop library and 188 core tests plus integration/guardrails), `git diff --check`, clean worktree checks, and `make lint` passed.

### Summary

PASSED: correction `f9bee09` resolves the prior Unicode display-width rejection without regressing the accepted ASCII, input, feedback, OSC 52, ownership, or read-only behavior. The full underlying slug remains copyable even when a wide Unicode ID is visibly truncated at the responsive column boundary.
