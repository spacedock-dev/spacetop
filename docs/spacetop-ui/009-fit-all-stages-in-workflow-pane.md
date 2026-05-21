---
id: "009"
title: Fit all workflow stages within the workflow pane
status: review
source: captain
started: 2026-05-21T02:52:59Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-009-fit-all-stages-in-workflow-pane
issue:
pr:
mod-block: merge:pr-merge
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

