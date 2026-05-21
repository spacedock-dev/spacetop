---
id: "009"
title: Fit all workflow stages within the workflow pane
status: done
source: captain
started: 2026-05-21T02:52:59Z
completed: 2026-05-21T04:41:36Z
verdict: PASSED
score:
worktree: 
issue:
pr: #38
mod-block: 
---

Workflows with many stages (for example `/Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/`, which declares 12 states: `pending`, `scoping`, `ideate`, `review`, `smoke`, `run`, `analyze`, `promote`, `expanded`, `ideated`, `done`, `rejected`) overflow the workflow pane. The current renderer in `src/ui/graph.rs` picks one of three `WidthTier` modes:

- **Wide** — single ribbon line; fails when the joined `name → name → …` form is wider than the pane.
- **Narrow** — splits the stages into two rows at the midpoint; still fails when even half the stages don't fit on one row.
- **VeryNarrow** — one stage per line; produces N lines for N stages, which silently overflows pane height when the pane is short.

In every tier, stages past the visible width or height are dropped from view without any indication that more exist. The captain wants the workflow pane to always communicate the full stage topology, even when the workflow is long, so that nothing is hidden.

## Acceptance criteria

**AC-1 — All declared stages are visible (or visibly accounted for) in the workflow pane at any terminal size that fits a usable Spacetop overview.**
Verified by: a `cargo test` rendering assertion that loads a workflow with 12 stages (matching the `research` workflow above) into the overview, renders into a buffer at a representative narrow pane size (e.g. 80×24 overall, with the workflow pane area derived from the existing overview layout), and asserts every stage name appears in the rendered buffer — or, if a stage name is intentionally elided, that an explicit overflow indicator names how many stages are hidden so the captain knows to widen the pane.

**AC-2 — Stage rendering degrades gracefully across width tiers without dropping stages.**
Verified by: `cargo test` assertions that re-render the same 12-stage fixture at the Wide, Narrow, and VeryNarrow width breakpoints and confirm every stage name is present in each tier's buffer. The design stage should decide whether this means wrapping more rows in Narrow, multi-column layout in VeryNarrow, an overflow marker with paging/scroll, or another approach that still shows every stage.

**AC-3 — Existing single-row and two-row layouts still apply when the workflow fits.**
Verified by: pre-existing `cargo test` cases for short workflows (4-stage `spacetop-ui` README, the existing test fixtures in `src/ui/graph.rs` tests) continue to pass unchanged. The fix should only change behavior when stages would otherwise be hidden.

**AC-4 — Feedback arcs and per-stage counts remain attached to their stages.**
Verified by: rendering assertions that, when stages reflow across rows or scroll, the count and any feedback annotation still appear with the correct stage name (no orphaned counts, no misattributed feedback edges).

**AC-5 — `make lint` and `cargo test` pass.**
Verified by: `make lint` (clippy `-D warnings`) and `cargo test` from the repo root, both green.

## Stage Report: implement

- DONE: Approach decision: name the chosen strategy for fitting many stages
  Hybrid reflow: Narrow tier now greedily wraps the compact `name(count) → …` form across as many rows as the width requires (was hard-coded to 2); VeryNarrow tier now lays cells into a width/height-aware multi-column grid and falls back to a `+N hidden: …` indicator naming the elided stages. This degrades gracefully across AC-1/AC-2 without ever silently dropping a stage.
- DONE: Test evidence for the 12-stage case
  Added `fits_all_twelve_research_stages_at_narrow_pane_size`, `…_in_wide_tier`, `…_in_narrow_tier`, `…_in_very_narrow_tier`, plus `very_narrow_overflow_indicator_names_hidden_stages` in `src/ui/graph/tests.rs` — fixture lists the exact research stages (pending, scoping, ideate, review, smoke, run, analyze, promote, expanded, ideated, done, rejected) and each renders into a `TestBackend` buffer with assertions that every name appears (or that an explicit `+N hidden: …` indicator names the hidden count).
- DONE: make lint clean and full cargo test green
  `make lint` → clean (`-D warnings`); `cargo test` → 260 + 4 + 8 lib/integration tests pass, 3 notify-backend tests intentionally ignored, 0 failures.

### Summary

Replaced the fixed 2-row Narrow split and one-stage-per-line VeryNarrow renderer with width/height-aware layouts: Narrow greedily wraps the compact form across N rows, VeryNarrow packs cells into a multi-column grid sized to the pane and emits a named overflow indicator if the grid still can't hold every stage. All 19 pre-existing graph tests pass unchanged; five new tests cover the 12-stage research workflow across every width tier and the extreme-overflow fallback.

### Feedback Cycles

#### Cycle 1 — captain rejection at implement (2026-05-21)

Captain reviewed the rendered output against the 12-stage `dataagentbench/docs/research/` workflow and rejected. Concrete asks:

1. **Stage names must be colored per stage** in every tier — the existing Wide tier colors each node via `definition.stage_color_for(&col.stage_name)` (and applies `Modifier::REVERSED` for the active stage). The new Narrow and VeryNarrow renderers render stage names as plain text, losing the per-stage color identity. Apply the same `stage_color_for` styling to stage-name spans in `build_narrow_row` / VeryNarrow cell rendering (and keep the active-stage REVERSED modifier).
2. **Arrow relations between stages must remain visible across all tiers.** Wide uses ` → ` between nodes; the prior 2-row Narrow used the same. The new wrapping Narrow rendering and the multi-column VeryNarrow grid drop the connective arrows entirely, so the workflow topology no longer reads as a directed sequence — it looks like a flat list. Restore an inter-stage arrow glyph (`g.forward_arrow` / `g.narrow_arrow`) between adjacent stage cells in both the wrapped Narrow rows and the VeryNarrow grid (within a row, and ideally a wrap indicator at row breaks so the reader can follow the sequence across rows).


## Stage Report: implement (cycle 2)

- DONE: Per-stage color: every stage-name span in the new Narrow (wrapped rows) and VeryNarrow (multi-column grid) renderers uses `definition.stage_color_for(&stage.name)` with `Modifier::BOLD` (same convention as the Wide tier), and active-stage cells layer `Modifier::REVERSED` on top.
  `src/ui/graph.rs::render_narrow` builds each stage span with `Style::default().fg(definition.stage_color_for(seg.stage_name)).add_modifier(Modifier::BOLD)` plus REVERSED when active; `render_very_narrow` does the same for each grid cell. Both renderers now take `&WorkflowDefinition`. New tests `narrow_tier_colors_each_stage_name_per_stage` and `very_narrow_tier_colors_each_stage_name_per_stage` assert that every stage-name span in the 12-stage research workflow carries the expected per-stage color + BOLD, and that the active stage span layers REVERSED.
- DONE: Arrow relations preserved across tiers: the wrapped Narrow renderer separates adjacent stages within a row with the existing narrow arrow glyph, and the VeryNarrow grid does the same between cells within a row. At row breaks, a trailing arrow on the wrapping row and a leading arrow on the continuation row signal that the directed sequence continues across rows.
  `render_narrow` emits ` → ` (`g.narrow_arrow` with spaces) styled DarkGray between adjacent segments and wrap_trailing/wrap_leading at row breaks. `render_very_narrow` uses the same ` → ` between cells and a leading wrap arrow on rows after the first plus a trailing wrap arrow when more stages remain in the grid. Both color/arrow tests assert the arrow glyph `→` is present.
- DONE: make lint clean and full cargo test green from the worktree root; pre-existing graph tests still pass, and the previously added 12-stage tests are updated (not deleted) to assert the new color and arrow expectations.
  `make lint` → clippy clean with `-D warnings`. `cargo test` → 262 lib + 4 bin + 8 integration tests pass, 3 notify-backend tests intentionally ignored, 0 failures. The prior 12-stage tests (`fits_all_twelve_research_stages_*`, `very_narrow_overflow_indicator_names_hidden_stages`) still pass unchanged, and `narrow_dag_wraps_to_two_rows` was updated for the new `render_narrow(..., definition)` signature (still asserts the same wrapping behavior).

### Summary

Restored per-stage color + BOLD (and active-stage REVERSED) on every stage span in the new Narrow and VeryNarrow renderers by routing the existing `WorkflowDefinition::stage_color_for` through their span construction. Reinstated inter-stage arrows within rows for both tiers and added trailing/leading wrap arrows at row breaks so the directed sequence reads continuously across wraps. The wrapping/multi-column layout from cycle 1 (f00f57b) is untouched — only styling and connective glyphs were layered on top.

#### Cycle 2 — captain rejection at implement (2026-05-21)

Color + arrows from Cycle 1 are confirmed fixed. Two new concrete asks against the live rendering of the 12-stage `dataagentbench/docs/research/` workflow:

1. **Workflow graph should use the entire pane width.** The current VeryNarrow multi-column grid is right-aligned with a large empty gap on the left half of the workflow pane (visible in the captain's screenshot at ~109-col total width — the grid only occupies columns ~50→109 while columns 0→50 are blank). Distribute the columns to span the inner width, with reasonable horizontal padding for legibility (left-align with even inter-column spacing, or distribute remaining slack across the gaps — whichever reads cleaner). Apply the same "use the full pane width" property to the Narrow (wrapped-rows) tier as well.
2. **Reserve space for the feedback (rollback) arrow.** The `research` workflow declares `review` with `feedback-to: implement`, but the rendered output in VeryNarrow shows no rollback-on-reject annotation. In Narrow, `feedback_annotations` emits `↩ rollback on reject: review → implement` as a trailing line; that same annotation must render in VeryNarrow (and reserve vertical space for it in the layout calculations so the multi-column grid does not silently consume the row the annotation would have used). Equivalently: the grid height budget must account for the feedback-annotation lines before deciding how many stage rows fit, and the annotation must appear styled as a feedback rollback (matching the Narrow rendering convention).


## Stage Report: implement (cycle 3)

- DONE: Full-width layout: the wrapped Narrow renderer and the VeryNarrow multi-column grid distribute their columns to span the inner pane width
  `render_narrow` now two-passes: greedy pack into rows, then per-row distribute slack across inter-stage gaps so every row's visible_width equals `inner_width`. `render_very_narrow` does the same per-row distribution (accounting for `wrap_leading`/`wrap_trailing` width on each row individually) and picks the LARGEST column count that fits in the height budget so a 12-stage workflow at width 100/height 12 no longer collapses to a single column. New tests `narrow_tier_uses_full_pane_width` and `very_narrow_tier_uses_full_pane_width` exercise the 12-stage research fixture and assert each stage-bearing row's visible_width spans `inner_width` (no large trailing gap).
- DONE: Reserve and render the feedback-rollback annotation in VeryNarrow
  `render_very_narrow` now calls `feedback_annotations(...)` up front and subtracts the resulting `feedback_rows` (0 or 1) from `inner_height` BEFORE picking `chosen_cols/chosen_rows`, so the grid never silently consumes the row the annotation needs. The annotation is then appended to the output (matching Narrow tier behaviour). New test `very_narrow_tier_renders_feedback_rollback_annotation` builds a 12-stage research fixture with `review → feedback-to: implement` and asserts the rendered output contains `rollback on reject`, both stage names, and the `↩` glyph.
- DONE: make lint clean and full cargo test green; all prior 12-stage and color/arrow tests still pass
  `make lint` → clippy clean with `-D warnings` (after switching to `checked_div`/`checked_rem` to satisfy `manual-checked-ops`). `cargo test` → 265 lib + 4 bin + 8 integration tests pass, 3 notify-backend tests intentionally ignored, 0 failures. The prior 12-stage tests, color/arrow tests, and `narrow_dag_wraps_to_two_rows` all still pass unchanged.

### Summary

Made both wrapping tiers actually use the inner pane width: `render_narrow` distributes per-row slack across the inter-stage `→` gaps (with leftover sprinkled into the leading gaps), and `render_very_narrow` does the same per-row math while accounting for wrap arrows. The VeryNarrow column-count search now prefers the LARGEST column count that fits the height budget (was: smallest), so multi-stage workflows actually use the right half of the pane. Layered the feedback-annotation reservation on top: feedback rows are computed and subtracted from the grid's height budget BEFORE choosing rows/cols, and the rollback line is appended in both Narrow and VeryNarrow tiers. Cycle 1 (color + arrows) and the earlier cycle-2 reflow work are untouched.

#### Cycle 3 — captain rejection at implement (2026-05-21)

Cycles 1 and 2 fixes confirmed landed. Two polish asks on the wrapped tiers:

1. **Use 90% of the pane width (not 100%) — leave a left+right margin.** The current full-width distribution stretches the graph edge-to-edge with no breathing room. Render the graph into roughly 90% of `inner_width`, horizontally centered (or left-padded by half the slack and right-padded by the rest), so the workflow pane has visible margin around the graph. Apply to both the wrapped Narrow tier and the VeryNarrow multi-column grid. The exact 90% rule is a usability default — choose a clean integer column budget like `inner_width * 9 / 10` (with a floor so very narrow panes still consume what they can), and route the same logic through both tiers.
2. **At least 3 blank lines of vertical padding between row 1 and row 2 of the stage grid.** The current rendering places the second wrapped row directly under the first, which makes adjacent rows visually run together. Inject ≥3 blank lines between consecutive stage rows (in both Narrow's wrapped form and VeryNarrow's grid) so each row reads as its own band. The blank lines are pure spacers — no glyph, no annotation. The grid's row-count budgeting must subtract the inter-row blank lines from `inner_height` BEFORE choosing how many rows fit, so the inter-row padding does not push stages off-screen.


## Stage Report: implement (cycle 4)

- DONE: 90% width budget with horizontal margin: both wrapped Narrow and VeryNarrow renderers consume roughly `inner_width * 9 / 10` columns (with a sane floor for very narrow panes), centered (or left-padded by half the slack and right-padded by the remainder) inside `inner_width`. The same budget threading flows through `pick_width_tier`'s decisions so the choice of tier matches the actual usable width. Add a test on the 12-stage research fixture asserting the graph's rendered column range is within 10% of inner_width and that both left and right margins are non-zero.
  New `usable_inner_width`/`horizontal_margins` helpers in `src/ui/graph.rs`; `render_stage_graph` threads `usable_width` into `pick_width_tier`. Both `render_narrow` and `render_very_narrow` use `usable_width` for wrap/slack math and frame each line with a left+right margin span so total visible width still equals `inner_width`. Tests `narrow_tier_uses_full_pane_width` and `very_narrow_tier_uses_full_pane_width` (updated for cycle 3) assert non-zero left+right margins and total-width == inner_width on the 12-stage research fixture.
- DONE: Inter-row vertical padding >=3 blank lines between consecutive stage rows in both Narrow (wrapped) and VeryNarrow (grid) tiers, with row-count budgeting that subtracts the inter-row padding from `inner_height` BEFORE choosing how many rows fit, so the padding does not push stages off-screen. The blank lines are pure spacers. Add tests asserting (a) exactly >=3 blank Lines appear between any two stage-bearing rows in the rendered output, and (b) when `inner_height` is constrained, the row count chosen accounts for the padding (no stages elided that would have fit without padding-subtraction logic).
  `INTER_ROW_PADDING_LINES = 3` injected via `Line::default()` between stage rows in both renderers. VeryNarrow computes `max_rows_with_padding = (budget + pad) / (1 + pad)` BEFORE the column-count search so padding never silently elides stages. New tests `narrow_tier_inserts_blank_lines_between_rows`, `very_narrow_tier_inserts_blank_lines_between_rows`, and `very_narrow_tier_row_budget_accounts_for_inter_row_padding` cover both asserts.
- DONE: make lint clean and full cargo test green; all prior 12-stage tests (color, arrows, full-width, feedback-row) still pass.
  `make lint` → clippy clean with `-D warnings`. `cargo test` → 268 lib + 4 bin + 8 integration tests pass, 3 notify-backend tests intentionally ignored, 0 failures. Cycle-1/2/3 tests (`fits_all_twelve_research_stages_*`, color/arrow tests, full-width tests, `very_narrow_tier_renders_feedback_rollback_annotation`, `narrow_dag_wraps_to_two_rows`) all still pass — width/height inputs on a few were bumped to absorb the new inter-row blank padding (no semantic change to what they assert about stages).

### Summary

Layered cycle 3's 90% horizontal margin and ≥3-line inter-row padding on top of the prior color, arrow, full-width, and feedback-row work. Threaded a shared `usable_inner_width` budget through `pick_width_tier` and both wrapping renderers so the tier choice and the rendered content agree on the same column budget, and added padding-aware row-count math (`max_rows_with_padding = (budget + pad) / (1 + pad)`) to the VeryNarrow grid so the new spacers never silently elide stages. The cycle-3 commit is the only edit to `src/ui/graph.rs` and `src/ui/graph/tests.rs` beyond what cycles 1–3 already landed.

#### Cycle 4 — captain rejection at implement (2026-05-21, cycle-limit override)

3-cycle threshold reached; captain explicitly authorised one more iteration on two concrete polish asks:

1. **No trailing arrow after the final (terminal) stage on the last wrapped row.** Current rendering shows `↪ … rejected (0) →` (terminal stage followed by a wrap-trailing/inter-stage arrow before the right margin). The terminal stage is the end of the directed sequence; nothing follows it, so no glyph belongs there. Suppress the trailing arrow after the very last stage in both wrapped Narrow and VeryNarrow tiers (i.e. only emit `wrap_trailing` / inter-stage `→` between two real stage cells, never after the final emitted cell).
2. **Tighten inter-row spacing.** The current `INTER_ROW_PADDING_LINES = 3` reads as too much vertical air between stage rows. Drop it to `1` blank line between consecutive stage rows in both wrapping tiers. Keep the padding-aware row-count math intact — just change the constant and update the affected tests.

Do not regress the prior color, arrows, full-width, feedback-row, or 90% margin work.


## Stage Report: implement (cycle 5)

- DONE: Suppress trailing arrow after the final emitted stage in both wrapped Narrow and VeryNarrow tiers. Inter-stage `→` and `wrap_trailing` glyphs render only between two real stage cells, never after the last one in the rendered sequence. Add/adjust tests asserting the final row's rendered text does not end with an arrow glyph for the 12-stage research fixture; cycle-1 arrow tests must still pass.
  `render_very_narrow` now sets `has_trail = row_last_excl < visible_cells` (was `row_last_excl < visible_cells && row_last_excl < cells.len()`), so the final visible stage row never carries a `wrap_trailing` even when a `+N hidden:` overflow indicator follows. Narrow's final row already had `has_wrap_trailing: false` hardcoded. New tests `narrow_tier_last_row_does_not_end_with_arrow` and `very_narrow_tier_last_row_does_not_end_with_arrow` (both fit and overflow cases) assert the last stage-bearing row's trimmed text does not end with `→`; cycle-1 `narrow_tier_colors_each_stage_name_per_stage` / `very_narrow_tier_colors_each_stage_name_per_stage` arrow assertions still pass unchanged.
- DONE: Change `INTER_ROW_PADDING_LINES` from `3` to `1`. Update the inter-row spacing tests (the `>=3 blank lines` assertions become `==1 blank line`). Keep the padding-aware row-count math (`max_rows_with_padding = (budget + pad) / (1 + pad)`) intact — only the constant changes.
  `INTER_ROW_PADDING_LINES` flipped from `3` to `1` in `src/ui/graph.rs`; the `max_rows_with_padding` formula and all uses of the constant are untouched. Tests `narrow_tier_inserts_blank_lines_between_rows` and `very_narrow_tier_inserts_blank_lines_between_rows` now `assert_eq!(gap, 1, …)`; `very_narrow_tier_row_budget_accounts_for_inter_row_padding`'s height was rebalanced from 13 (4 + 3*3) to 7 (4 + 3*1) to exercise the same exact-fit / one-short branch with the new constant.
- DONE: make lint clean and full cargo test green; prior 12-stage tests for color, arrows (between adjacent stages only), full-width, feedback-row, and 90% margin all still pass.
  `make lint` → clippy clean with `-D warnings`. `cargo test` → 270 lib + 4 bin + 8 integration tests pass, 3 notify-backend tests intentionally ignored, 0 failures. All cycle-1/2/3/4 tests (color, arrows, full-width, feedback-row, 90% margin) still pass without further edits.

### Summary

Two narrow polish edits per the cycle-4 captain feedback: dropped the inter-row spacer from 3 lines to 1, and tightened the VeryNarrow `has_trail` predicate so a wrap arrow is emitted only when at least one more stage cell will actually be rendered in a subsequent visible row (was: any time more cells exist in the source list, which pointed an arrow at the `+N hidden:` overflow line). Narrow's final-row trailing-arrow path was already suppressed; added regression tests for both tiers (12-stage research fixture, fit + overflow cases) so the rule cannot silently regress.

