---
id: "010"
title: Render the workflow graph as a true ASCII DAG with line-drawing edges
status: review
source: captain
started: 2026-05-21T05:29:51Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-010-render-workflow-as-ascii-dag
issue:
pr:
mod-block: merge:pr-merge
---

Today the workflow pane (after task 009) renders stages as a wrapped linear sequence: `pending → scoping → ideate → review → … `, possibly wrapped onto multiple rows with `↪` continuation arrows. This reads as a strip, not a directed acyclic graph. Workflows with feedback edges (e.g. `review` with `feedback-to: implement`) get a small "rollback on reject" annotation line at the bottom, but the feedback edge itself is not drawn.

The captain wants the workflow pane to look like an actual ASCII DAG: stages as plain inline nodes with line-drawing edges (`─`, `▶`, `╭`, `╯`, `│`) connecting them, and feedback edges drawn as visible arcs looping back from the source stage to its `feedback-to` target — not annotated as a footer line. The chosen style (per captain preview) is **plain nodes + line-drawing edges**, no boxes around stage names.

Reference layout (captain-confirmed preview, for a workflow with `review → feedback-to: implement` plus a forward chain):

```
pending(8) ──▶ scoping(1) ──▶ ideate(0) ──▶ review(0)
                                                     │
  ╭────── (feedback on reject) ──────╮         ▼
  │                                  │         smoke(0)
implement(0) ◀────────────────────────╯         │
                                                     ▼
                                                  run(0)
```

The exact layout (column positions, where to break, how to route the arc) is a design-stage concern. The 009 work landed the basic primitives the design stage can build on: per-stage color (`stage_color_for`), inter-stage arrow glyphs, width/height-aware budgeting, and a 90% inner-width margin.

## Background

- 009 (just merged via PR #38) extended `src/ui/graph.rs` with `render_narrow` (wrapped) and `render_very_narrow` (multi-column grid) tiers, both linear. Wide tier renders a single ribbon line with no wrapping. None of the tiers draw an explicit edge between stage nodes — adjacency is implied by the `→` separator only.
- Feedback edges currently surface via `feedback_annotations(stages, g)` in `src/ui/graph.rs`, which returns text like `↩ rollback on reject: review → implement` rendered as a trailing Line.
- The 12-stage `dataagentbench/docs/research` workflow remains the canonical many-stages fixture. The 4-stage `spacetop-ui` README is the canonical short-workflow fixture.

## Acceptance criteria

**AC-1 — Stage nodes are connected by drawn edges, not implied by separators.**
Verified by: a `cargo test` that renders the spacetop-ui workflow (4 stages, with `review → feedback-to: implement`) into a TestBackend buffer and asserts the rendered output contains line-drawing characters from the box-drawing block (e.g. at least one of `─`, `│`, `╭`, `╯`, `▶`) connecting adjacent stage nodes — not as a separator span, but as a horizontal/vertical edge that the eye reads as "node A connects to node B".

**AC-2 — Feedback edges render as drawn arcs, not as a footer annotation.**
Verified by: the same test asserts that for the `review → feedback-to: implement` edge there is a visible looping arc (combination of `╭`/`╮`/`╯`/`╰` + `─` + `│` + an arrowhead `◀` or `▶`) in the rendered buffer, AND that the prior `↩ rollback on reject: …` footer line is no longer emitted. (Or, if the design stage decides to keep a textual fallback, the fallback only renders when the arc cannot be drawn due to width/height constraints — and the test covers both cases.)

**AC-3 — DAG rendering respects the inner pane width/height budget and degrades when constrained.**
Verified by: tests that render the 12-stage research fixture at a constrained pane size and assert either (a) the DAG fits within the pane with the same 90% width-margin contract from 009, or (b) when the DAG cannot fit, an explicit "+N nodes hidden" / "+N edges hidden" indicator names the missing pieces, matching the 009 overflow-naming pattern. The design stage picks which one; the test covers it.

**AC-4 — Per-stage color, count, and active-stage highlighting are preserved.**
Verified by: tests asserting each stage span in the DAG carries `definition.stage_color_for(&stage.name)` + BOLD (and REVERSED for the active stage), and the `(count)` suffix renders alongside the stage name with the same correctness as 009.

**AC-5 — Short workflows still render readably.**
Verified by: a test on the 4-stage `spacetop-ui` workflow that asserts the DAG layout is at least as readable as the post-009 linear form — every stage name + count + the feedback edge from `review` to `implement` are all present, and the total rendered height does not exceed a reasonable bound (the design stage names the bound).

**AC-6 — `make lint` and `cargo test` pass.**
Verified by: `make lint` (clippy `-D warnings`) and `cargo test` from the repo root, both green. Pre-009 graph tests and the 009-added 12-stage tests are either preserved unchanged or updated in lockstep with the new DAG renderer (the design stage decides; whichever path it picks, the tests must end green).

## Design-stage scope (for the next dispatched worker)

The design stage should:

1. Pick the **DAG layout algorithm**: simple topological columns (left-to-right by stage order) is the obvious default; the worker can propose something smarter if it justifies the complexity.
2. Pick the **edge routing strategy** for feedback arcs: route under the chain, over the chain, or alongside — choose whichever stays readable when multiple feedback edges exist.
3. Pick the **degraded-mode behavior** when the pane is too small to fit the DAG: fall back to the 009 wrapped linear form (recommended — preserves the captain's "show everything" contract) or render an explicit truncation indicator.
4. Name the **new helpers** to add to `src/ui/graph.rs` (or a new `src/ui/dag.rs` if the design warrants its own module) and the **existing helpers** that can be reused.
5. Confirm **whether `render_wide` / `render_narrow` / `render_very_narrow` survive** as fallback tiers or get replaced by the DAG renderer at every width. Recommendation: keep them as the degraded-mode fallback per (3).

The design output is the input to the implement stage — name code locations, sketch the rendered output, and write the acceptance assertions concretely enough that the implement worker can write the tests verbatim.

## Stage Report: implement

- DONE: Approach decisions called out in the stage report
  DAG layout: simple topological columns left-to-right with inline `(count)`
  collapsed into the node text (saves the dedicated counts row from 009).
  Feedback-edge routing: arcs render UNDER the chain via the existing
  `render_feedback_row` helper (rounded corners ╰/╯ + vertical bar │ + ↑
  arrowhead at the target column) — same geometry the captain confirmed in
  the entity preview. Degraded-mode: when the inline DAG node row exceeds
  the 90% usable_width, dispatch falls back to the 009-era wrapped
  `render_narrow` (and then `render_very_narrow`) tiers, preserving the
  "show everything" contract along with the `↩ rollback on reject:`
  footer that those tiers already carry. The `render_wide`/`render_narrow`/
  `render_very_narrow` tier names survive — Wide now routes to `render_dag`;
  Narrow and VeryNarrow are unchanged.
- DONE: Test evidence for AC-1..AC-5
  AC-1+AC-2: `dag_spacetop_ui_renders_drawn_edges_and_feedback_arc` asserts
  ─, ►, ╰, ╯, │, ↑ are all present for the 4-stage spacetop-ui fixture.
  `dag_does_not_render_rollback_footer_when_arc_is_drawn` locks the absence
  of the legacy `↩ rollback on reject:` footer in DAG mode. AC-3:
  `dag_twelve_stage_research_fits_or_names_hidden_stages` exercises the
  12-stage fixture at 80x7 — degraded fallback kicks in and every stage
  name remains visible. AC-4:
  `dag_each_stage_span_carries_per_stage_color_and_bold` asserts
  per-stage `stage_color_for` + BOLD + REVERSED for the active stage on
  every DAG span. AC-5: `dag_short_workflow_stays_within_height_bound`
  pins the 4-stage rendered height at ≤4 lines (1 chain + 2 arc + 1
  spare) and re-asserts every stage name + `(count)` + the drawn arc
  corners + absence of the footer. The pre-009 marker tests
  (`layout_columns_*`) were retargeted from `layout_columns` to
  `dag_layout_columns` (the new layout function); behaviour is unchanged
  so the existing assertions still hold. No 009 tests were dropped.
- DONE: make lint clean and full cargo test green
  `make lint` finished without diagnostics; `cargo test` reports
  283 + 4 + 10 = 297 unit/integration tests pass (graph::tests
  39 passed, 0 failed) with 3 notify-backed watcher tests ignored as
  before.

### Summary

The DAG tier (`render_dag`) replaces `render_wide`. It builds inline
`{leading?} {name}({count}){terminal?}` nodes via `build_dag_node_text` +
`dag_layout_columns`, connects them with `──▶`, draws feedback arcs UNDER
the chain via the existing `render_feedback_row` helper, and preserves the
009 horizontal-margin contract. When the inline chain cannot fit
`usable_inner_width`, dispatch falls back to the 009 wrapped
`render_narrow`/`render_very_narrow` tiers — which still carry the
`↩ rollback on reject:` footer so the "show everything" contract from 009
holds. Five new tests cover AC-1..AC-5; 34 prior tests still pass
unchanged. Dead helpers (`render_wide`, `build_counts_line`,
`style_counts_spans`, `padded_feedback_lines`, `padded_styled_line`,
`layout_columns`) and the now-unused `ColumnLayout::count` field were
removed; `make lint` (`-D warnings`) and `cargo test` are both green.

## Feedback Cycles

### Cycle 1 — captain rejection at implement (2026-05-21)

Two material gaps surfaced when the captain installed the worktree binary and ran it against real workflows:

1. **The DAG must wrap AS A DAG when the chain does not fit the pane width — it must not fall back to the 009 wrapped-text tiers.** At ~109 cols with the 12-stage `dataagentbench/docs/research` workflow, the inline single-row chain `pending(7) ──▶ scoping(0) ──▶ … ──▶ rejected(0)` cannot fit, so `render_dag` currently bails to `render_narrow` and the captain sees the pre-010 wrapped-text rendering — visually indistinguishable from the post-009 output. The captain's confirmed preview shows the chain wrapping ACROSS ROWS with drawn `│`/`╭`/`╮`/`╯`/`╰` glyphs connecting one row to the next AND with feedback arcs drawn alongside. The fix: `render_dag` must support multi-row DAG layout (greedy pack nodes into rows that fit `usable_width`; between rows emit drawn `╮` (right turn down) + `│` + `╰` (left turn into next row) so the directed sequence reads continuously). Only fall back to the 009 wrapped-text tiers when the multi-row DAG itself cannot fit the available height — and even then, prefer surfacing an explicit overflow indicator (`+N nodes hidden`) over silently switching renderer families.

2. **The feedback arc is broken: corner glyphs and horizontal segments are missing — only `↑`, the `reject` label, and a stray `│` render.** Captain screenshot for the spacetop-dev workflow (5 stages: `design → plan → implement → review → done` with `review → feedback-to: implement`) shows the second row as `↑    reject    │` — the `↑` is under the `implement` column, the `│` is under the `review` column, but the connecting `╰─────reject─────╯` arc between them is absent. The drawn arc must actually connect source-column → target-column: source emits `│` then `╰` (or `╮` if routed under, `╯` if routed over) at the row break; horizontal `─` segments span the columns between target and source with the label centered; target emits the matching corner and `↑` arrowhead. Cite a unit test that renders the spacetop-dev fixture and asserts the full glyph sequence on the arc row (corner → `──reject──` → corner → arrowhead), not just the endpoints.

Both gaps are in `render_dag` / `render_feedback_row` (or whatever helper draws the arc). Do NOT remove the 009 wrapped tiers — they remain a deeper fallback for the height-constrained case after multi-row DAG has been exhausted.

## Stage Report: implement (cycle 1)

- DONE: Multi-row DAG layout: when the inline single-row chain exceeds `usable_inner_width`, `render_dag` wraps the chain across as many rows as needed, drawing connective glyphs (right-turn-down at end of row N, vertical bar, left-turn-into-row-N+1) so the directed sequence reads continuously across rows.
  Introduced `dag_layout_rows` + `DagRowPlan` + `build_row_break_connector` in `src/ui/graph.rs`. Each chain row ends with `──╮` when followed by another row; the connector line below carries `╭` at the next row's start column, `─` fill across, and `╯` directly under the prior row's `╮`. `pick_width_tier` now consults height via `dag_total_line_count` so the multi-row DAG is preferred whenever it fits — Narrow/VeryNarrow are entered only when the multi-row DAG itself overflows. New test `dag_multi_row_wraps_with_drawn_connectors_on_research_fixture` covers the research 12-stage fixture at 100x10: every stage name + `(0)` count visible, `╭`/`╮`/`╯`/`─` connector glyphs present, and the chain renders the wide `►` arrow (not the Narrow tier's `→`).
- DONE: Drawn feedback arc with all segments: the arc from source stage to its `feedback-to` target renders the full glyph sequence — vertical bar from source column, corner turn, horizontal `─` spans across the columns between target and source with the label centered, matching corner at target column, and `↑`/`↓`/`◀`/`▶` arrowhead at the target.
  Fixed arc row ordering in `render_dag`: the arrow row (`↑ reject │`) now renders FIRST (closer to chain) and the corner row (`╰────╯`) SECOND below it, so the visual flow drops from source `│` down to corners then re-enters target via `↑`. Added `dag_drawn_feedback_arc_is_fully_connected_on_spacetop_dev` which inspects the rendered buffer at 109x10 for the spacetop-dev fixture: it locates the arrow row by `↑` + `│` + `reject`, derives target_col and source_col from the arrow positions, then asserts the very next row carries `╰` at target_col, `╯` at source_col, AND every column between target_col+1 and source_col-1 is `─`. Endpoints alone (the cycle-0 bug) would fail this test.
- DONE: make lint clean and full cargo test green; the 5 cycle-1 (cycle-0?) DAG tests added in commit 8dd2c8a are either updated in lockstep or kept passing alongside the new multi-row + connected-arc tests.
  `make lint` (`-D warnings`) clean. `cargo test` 286 lib + 4 + 10 integration = 300 tests pass (3 notify-backed watcher tests ignored as before). All 5 cycle-0 DAG tests (`dag_spacetop_ui_renders_drawn_edges_and_feedback_arc`, `dag_does_not_render_rollback_footer_when_arc_is_drawn`, `dag_twelve_stage_research_fits_or_names_hidden_stages`, `dag_each_stage_span_carries_per_stage_color_and_bold`, `dag_short_workflow_stays_within_height_bound`) pass unchanged. Two pre-cycle tests (`narrow_tier_renders_compact_textual_summary`, `very_narrow_tier_renders_feedback_rollback_annotation`) had their pane heights tightened (4x4 / 40x9) because the multi-row DAG now consumes the wider/taller cases those tests used to target — the underlying tier behaviour they pin is otherwise unchanged. Three new tests added: `dag_multi_row_wraps_with_drawn_connectors_on_research_fixture`, `dag_falls_back_to_009_wrapped_text_when_height_starved`, `dag_drawn_feedback_arc_is_fully_connected_on_spacetop_dev`.

### Summary

`render_dag` now packs nodes into multi-row layouts via `dag_layout_rows` and connects rows with drawn `──╮`/`╭───╯` glyphs (using two new entries in `GlyphSet`: `arc_corner_down_right` = ╭ and `arc_corner_down_left` = ╮). `pick_width_tier` takes `inner_height` into account so the DAG tier wins whenever it can pack everything; the 009 wrapped-text tiers are entered only when the multi-row DAG would overflow the available rows. The feedback-arc row order was flipped so the arrow row renders directly under the chain and the corner row below it — giving the eye a continuous flow from source `│` down through the corners and back up via `↑` at the target. Two new tests cover the multi-row connector glyphs and the fully-connected drawn-arc sequence respectively; one regression test pins the deeper-fallback path. Lint and the full test suite remain green.

### Cycle 2 — captain rejection at implement (2026-05-21)

Arc rendering from Cycle 1 is confirmed fixed (full connected glyph sequence visible). One alignment ask:

**Center-align the DAG horizontally within the pane.** Current rendering left-aligns the DAG inside the 90% usable-width band — at typical pane widths the chain (e.g. spacetop-dev's 5 stages) occupies the left half of the pane and leaves the right half empty. The captain wants the DAG centered horizontally: compute the chain's actual rendered width and pad both sides so the chain sits in the middle of `inner_width` (or middle of `usable_inner_width`). Apply to both single-row and multi-row DAG layouts. The feedback-arc row must shift with the chain so the corners and arrowhead remain column-aligned with their source/target stages. The Narrow/VeryNarrow fallback tiers from 009 are NOT in scope for this change — they already distribute slack across the row internally via the 009 cycle 2 work.

Do not regress the multi-row DAG layout or drawn-arc-segments work from cycle 1.


