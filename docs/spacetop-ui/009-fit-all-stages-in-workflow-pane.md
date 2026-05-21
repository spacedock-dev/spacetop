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
