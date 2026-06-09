---
id: "049"
title: "Dynamic ID column width in the task list (slug-ID overflow fix)"
status: review
source: captain
started: 2026-06-09T02:55:50Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-049-dynamic-id-column-width
issue:
pr: "#48"
mod-block:
---

The task list ID column is hardcoded to `{:>4}` chars in `src/ui/list.rs`, which fit numeric IDs (`047`, `048`) but overflows for slug IDs (`adversarial-review`, `roadmap-v5`), causing the Title column to misalign. The fix is a dynamic column width: measure the longest ID in the visible item list on each render and size the column to fit, so Title always aligns regardless of ID style.

Flagged as a follow-up in task 048's plan and implement stage reports.

## Implementation plan

### Root cause

`src/ui/list.rs:136` formats every ID with a hardcoded width:

```rust
let id_str = format!("{:>4}", item.id);
```

The ID column is therefore 4 chars wide regardless of content. A slug ID
(`adversarial-review` = 18 chars, `roadmap-v5` = 10 chars) blows past 4 chars,
so the `Span` is longer than the column reserves. Because all later spans
(`"  "` separator at line 165, worktree marker, Title) are laid out by
concatenation — not absolute offsets — the Title start column shifts right by
`max(0, id.len() - 4)` on each row. Rows with different ID lengths get different
Title offsets, which is the visible misalignment.

The phase column already solved the identical problem: lines 111-116 compute a
per-render `pcw` (phase col width) from the longest visible status and feed it
to the `phase_col()` helper. We mirror that exact pattern for the ID column.

### Changes (all in `src/ui/list.rs`)

1. **Add a per-render ID width, alongside the existing `pcw` computation
   (after line 116).** Insert:

   ```rust
   // ID column width: widest visible ID, floored at 4 so numeric-ID
   // workflows (047, 048) are visually unchanged (AC-2). No upper clamp —
   // slug IDs may be long and the Title simply starts further right.
   let icw = items
       .iter()
       .map(|item| item.id.chars().count())
       .max()
       .unwrap_or(4)
       .max(4);
   ```

   Use `chars().count()` (not `.len()`) to match the existing `pcw` code and
   stay correct for any multi-byte ID; the spec's `.len()` in AC-1 is
   "or equivalent" and `chars().count()` is the established convention in this
   file.

2. **Replace the hardcoded format at line 136.** Change:

   ```rust
   let id_str = format!("{:>4}", item.id);
   ```
   to:
   ```rust
   let id_str = format!("{:>width$}", item.id, width = icw);
   ```

   Right-alignment is preserved (numeric IDs keep their current look). When
   `icw == 4` the output is byte-for-byte identical to the old code, so AC-2
   and the existing snapshot tests are unaffected.

No other spans change. The broken-entity rows (lines 199-220) carry no ID and
are unaffected. The fixed 4-char ID-column assumption documented in the
`task_row_no_glyphs_in_phase_col` comment (line 236-237) only matters for the
phase column it scans (x=2..14), which is left of the ID column and unchanged.

### Test strategy (no terminal backend required)

Two complementary tests in `src/ui/tests/task_list.rs`, following the existing
`phase_col_width_*` precedent (lines 365-481) which proves column-width logic
without a live terminal:

- **AC-2 — pure-logic floor test (`id_col_width_floors_numeric_ids_at_4`).**
  Build a `Vec<&WorkItem>` of numeric IDs (`"047"`, `"048"`) and assert the
  `icw` expression returns `4`, and that `format!("{:>4}", "047")` yields
  `" 047"` (4 chars) — i.e. numeric workflows are unchanged. This needs no
  terminal; it exercises the same arithmetic the render path uses. (Mirrors
  `phase_col_width_uniform_short_phases_clamped_to_4`.)

- **AC-1 — render alignment test (`task_row_title_aligns_with_slug_ids`).**
  Render via `TestBackend` (the harness already in this file: `app_with_items`,
  `item`, `find_text`, `buffer_text`) with two items whose IDs differ in length
  — e.g. `item("adversarial-review", "Title A", "Body")` and
  `item("v5", "Title B", "Body")`. The titles are distinct sentinels. Use
  `find_text(buffer, "Title A")` and `find_text(buffer, "Title B")` to get each
  title's start `x`, and assert **both titles start at the same column** (equal
  `x`). With the bug present the two titles land at different columns; with the
  fix the column is `icw`-padded so both align. This is the direct AC-1 proof:
  "the title starting at the correct offset" generalized to "all titles share
  one offset regardless of ID length". Width 100, height 24, matching sibling
  tests.

  A secondary assertion in the same test confirms the long slug ID
  `adversarial-review` renders in full (not truncated) via
  `buffer_text(...).contains("adversarial-review")`, proving the column grew to
  fit it.

### Verification commands

- **AC-1 + AC-2 (focused):**
  `cargo test --test '*' id_col_width task_row_title_aligns_with_slug_ids`
  is not reliable for inline `#[cfg(test)]` modules; instead run the two new
  tests by name:
  `cargo test id_col_width_floors_numeric_ids_at_4 task_row_title_aligns_with_slug_ids`
- **AC-3 (no regression):** `cargo test` (full suite) then `make lint`
  (clippy `-D warnings`, the project lint gate).

### File ownership for worktree execution

Single-file logic change (`src/ui/list.rs`) plus same-module tests
(`src/ui/tests/task_list.rs`). No parser/domain/app-state changes; the work is
contained entirely within the `ui` rendering layer. No new crate dependencies.

## Acceptance criteria

**AC-1 — Title column aligns when slug IDs are present.**  
Verified by: `src/ui/list.rs` computes `id_col_width = items.iter().map(|i| i.id.len()).max().unwrap_or(4).max(4)` (or equivalent) and uses it for both the ID and Title column offsets. Confirmed by a unit test or snapshot asserting the rendered row for `adversarial-review` has the title starting at the correct offset.

**AC-2 — Short numeric IDs still render at a minimum 4-char width.**  
Verified by: the dynamic width has a floor of `max(4, longest_id_len)` so `047`-style workflows are visually unchanged.

**AC-3 — No regression on existing task list tests.**  
Verified by: `cargo test` passes; `make lint` clean.

## Stage Report: plan

- DONE: Plan names the specific line(s) in src/ui/list.rs to change and the cargo test invocation that proves AC-1 and AC-2
  Names line 136 (`format!("{:>4}", item.id)` → `{:>width$}`) and the insertion point after line 116 for `icw`; verification cmd `cargo test id_col_width_floors_numeric_ids_at_4 task_row_title_aligns_with_slug_ids`.
- DONE: Test strategy identifies how to assert dynamic column width without a terminal backend
  Pure-logic floor test mirrors existing `phase_col_width_*` tests; render test uses the in-file `TestBackend` harness (`find_text`/`buffer_text`) to assert equal title `x` offset for differing ID lengths.
- DONE: Minimum-4-char floor for numeric-ID workflows is explicit in the plan (AC-2)
  Plan specifies `.max(4)` floor with no upper clamp; verified by exercise — `icw=4` for `["047","048"]` and `{:>4}`/`{:>width$}` outputs are byte-identical.

### Summary

Root cause is the hardcoded `{:>4}` ID width at src/ui/list.rs:136; the fix mirrors the existing phase-column `pcw` pattern (lines 111-116) with an `icw = items.iter().map(|i| i.id.chars().count()).max().unwrap_or(4).max(4)` and a `format!("{:>width$}", item.id, width = icw)`. Floor of 4 (no upper clamp) keeps numeric workflows byte-identical, satisfying AC-2. Both the floor arithmetic and the uniform-width alignment were proven by running a standalone rustc check rather than only asserting. Work is confined to the `ui` layer — one logic change plus two same-module tests, no parser/domain/app-state or dependency changes.

## Stage Report: implement

- DONE: `cargo test` passes — including id_col_width_floors_numeric_ids_at_4 (AC-2) and task_row_title_aligns_with_slug_ids (AC-1)
  Both new tests green (2/2). All 162 ui:: tests pass. Two suite failures (parser::load_archived_items_returns_entries_from_flat_files, app::toggle_scope_key_a_flips_to_archived_and_loads_lazily) are pre-existing _archive fixture drift — confirmed failing on the base commit cd66d64 with my changes stashed; untouched by this UI-only change.
- DONE: `make lint` passes — clippy -D warnings clean
  `make lint` finished with exit 0, no diagnostics.
- DONE: Title column x-offset is identical for rows with different-length slug IDs (the alignment bug is gone)
  task_row_title_aligns_with_slug_ids renders `adversarial-review` (18 ch) and `v5` (2 ch) and asserts both titles share one start column (alpha_x == beta_x); long slug ID renders in full, not truncated.

### Summary

Replaced the hardcoded `{:>4}` ID format at src/ui/list.rs with a per-render `icw = items.iter().map(|i| i.id.chars().count()).max().unwrap_or(4).max(4)`, mirroring the existing `pcw` phase-column pattern, and formatted the ID via `format!("{:>width$}", item.id, width = icw)`. Floor of 4 keeps numeric-ID workflows byte-identical (AC-2); no upper clamp lets slug IDs widen the column and shift all titles uniformly right (AC-1). Added two same-module tests in src/ui/tests/task_list.rs: a pure-arithmetic floor test and a TestBackend render test that proves uniform title alignment. Change is confined to the `ui` layer — no parser/domain/app-state or dependency changes.

## Stage Report: review

**VERDICT: PASSED**

- DONE: Verdict is explicit (PASSED or REJECTED) with per-AC evidence citations
  AC-1 PASSED: `task_row_title_aligns_with_slug_ids` (src/ui/tests/task_list.rs:507) renders `adversarial-review` (18 ch) and `v5` (2 ch) via TestBackend and asserts `alpha_x == beta_x` — both titles share one start column; long slug renders in full. Test green when run by name (`cargo test --lib task_row_title_aligns_with_slug_ids` → 1 passed). AC-2 PASSED: `id_col_width_floors_numeric_ids_at_4` (src/ui/tests/task_list.rs:486) asserts `icw == 4` for `["047","048"]` and `format!("{:>4}","047") == " 047"` — numeric layout byte-identical; green (1 passed). The render path at src/ui/list.rs:121-126 + :146 (`.max(4)` floor, no upper clamp, `{:>width$}`) is the exact code both ACs describe. AC-3 PASSED: full `cargo test` = 340 passed / 2 failed (pre-existing, see below); `make lint` exit 0, no clippy diagnostics.
- DONE: The two pre-existing test failures are confirmed unrelated to this change (not introduced by it)
  Checked out merge-base cd66d64 in a throwaway worktree with NONE of this task's diff present — both `parser::tests::load_archived_items_returns_entries_from_flat_files` and `app::tests::toggle_scope_key_a_flips_to_archived_and_loads_lazily` FAIL identically there. The parser failure is `_archive` fixture drift (assertion `items.iter().all(|item| item.status == "done")` at src/parser/tests.rs:301); the app test depends on archived-item loading. Neither touches the `ui` rendering layer this change is confined to.
- DONE: Review challenges whether the icw computation belongs in the render function or should be extracted, and rules on it
  RULED: keep it inline. `icw` has a single consumer (the `format!` at src/ui/list.rs:146) inside one function, and is a 5-line iterator chain with no reusable branching logic and no second call site — extraction would add indirection without removing duplication. It sits beside the pre-existing `pcw` (src/ui/list.rs:111-116), which is itself an inline per-render `let`; the only thing the phase column factors into a `phase_col()` helper is genuine logic (casing + ellipsization), which `icw` does not have. Inline matches the file's established convention and is the correct altitude.

### Summary

Independent review of the dynamic ID column-width fix. All three ACs PASS with cited test/exercise evidence: AC-1 (uniform title alignment across slug IDs) and AC-2 (4-char floor keeps numeric IDs byte-identical) are proven by the two new tests, both green when run individually against the lib crate; AC-3 holds — lint is clean and the only suite failures are the two pre-existing `_archive` fixture tests, confirmed failing identically at merge-base cd66d64 with this diff absent. The implementation is minimal, confined to `src/ui/list.rs` plus same-module tests, faithfully mirrors the existing `pcw` pattern, and the inline `icw` computation is at the right altitude — no extraction warranted. Recommend advancing to done.
