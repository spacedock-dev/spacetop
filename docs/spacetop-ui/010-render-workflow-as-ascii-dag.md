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
