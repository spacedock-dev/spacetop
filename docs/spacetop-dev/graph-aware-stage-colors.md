---
id: "013"
title: Assign stage colors using graph-aware coloring — no same color on adjacent stages
status: plan
source: captain feedback 2026-04-25
started: 2026-04-25T10:37:39Z
completed:
verdict:
score: 0.8
worktree:
issue:
pr:
---

The current `stage_color()` function in `src/ui/mod.rs` maps stage names to colors by name (hardcoded for known Spacedock names, deterministic hash fallback for unknowns). It does not consider the workflow graph structure at all. This means two stages that are directly connected by an edge — and therefore rendered side-by-side in the graph ribbon — can end up with the same color.

The rule: **if two stages have a dependency relationship (a direct edge between them in the stage graph), they must not share the same color.** Stages with no direct edge may share a color.

## Problem detail

The stage graph is a directed graph parsed from the workflow README frontmatter. Edges are:
- Linear progression: each stage → the next stage in `stages.states` order
- Feedback edges: a `feedback-to` field on a stage creates a reverse edge back to the named stage

For example, in a `design → plan → implement → review → done` workflow with `review feedback-to: implement`, the adjacency is:

```
design → plan → implement → review → done
                    ↑──────────────┘  (feedback)
```

Adjacent pairs that must differ: (design, plan), (plan, implement), (implement, review), (review, done), (implement, review) again via feedback.

With the current name-based scheme, `plan → Cyan` and `implement → Yellow` differ, but an unknown three-stage workflow could easily get the same palette index for stages 0 and 2 if stage 1 and stage 3 map to the same color via hash — which are not adjacent, so that's fine — but stages 0 and 1 could also collide on a different workflow.

## Proposed approach

Replace the stateless `stage_color(name) -> Color` call sites with a graph-aware color assignment that runs once per workflow load:

1. **Build adjacency from the stage list.** For stage at index `i`, add edges: `i → i+1` (linear). For any stage with `feedback_to`, add edge `i → feedback_to_index`.

2. **Run a greedy graph coloring** over the palette in stage order:
   - For each stage (in definition order), pick the lowest-index palette color not already used by any directly adjacent stage.
   - Palette: `[Blue, Yellow, Magenta, Green, Cyan, LightBlue, LightMagenta, Red]` — 8 colors, sufficient for any practical workflow.

3. **Store the assignment** in `WorkflowDefinition` or `WorkflowSnapshot` as a `Vec<Color>` (indexed by stage position). Pass it through to the render layer.

4. **The `stage_color(name)` function** either becomes a lookup into the precomputed assignment (keyed by name) or is replaced entirely. The current hardcoded name→color mapping for `design/plan/implement/review/done` can serve as an initial "preferred" palette hint to preserve familiar colors when there is no conflict.

## Implementation notes

- `src/domain.rs` — add `stage_colors: Vec<Color>` field to `WorkflowDefinition` or `WorkflowSnapshot`. Populate during `WorkflowSnapshot` construction.
- `src/parser.rs` (or wherever the snapshot is built) — implement the greedy coloring. Import `ratatui::style::Color` here, or keep color logic in `src/ui/` and compute the assignment there when building the overview state.
- `src/ui/mod.rs` — replace `stage_color(&item.status)` calls with a lookup into the precomputed map. The `stage_color` function can remain as a fallback for stages not in the current workflow (e.g., archived items from a different workflow).
- Keep the existing `stage_color` name-based function as a last-resort fallback for unknown stage names so archived-view rendering doesn't break.

**Preferred color hints** (preserve familiar colors when no conflict):
- `design` → Blue
- `plan` → Cyan
- `implement` → Yellow
- `review` / `feedback` → Magenta
- `done` / `complete` / `shipped` → Green
- `blocked` / `rejected` → Red

If a preferred color conflicts with an adjacent stage's assignment, fall back to the next available palette color for that stage.

## Acceptance criteria

**AC-1 — No two directly adjacent stages (linear or feedback edge) in the same workflow share the same color.**
Verified by: unit test that builds a workflow snapshot with a known adjacency (including a feedback edge) and asserts that `snapshot.stage_color(a) != snapshot.stage_color(b)` for every adjacent pair `(a, b)`.

**AC-2 — Non-adjacent stages may share a color (greedy coloring uses at most `ceil(max_degree + 1)` colors).**
Verified by: a 5-stage linear workflow uses at most 2 colors (since max degree = 2 in a path graph); test asserts the color set has at most 2 distinct values.

**AC-3 — Familiar stage names (`design`, `plan`, `implement`, `review`, `done`) keep their preferred colors when no conflict exists.**
Verified by: the standard `spacetop-dev` 5-stage workflow retains Blue/Cyan/Yellow/Magenta/Green respectively after the graph coloring pass.

**AC-4 — Existing graph-ribbon color tests pass with no regression.**
Verified by: `cargo test` exits 0; `graph_ribbon_uses_stage_colors_per_stage` and `stage_color_assigns_distinct_colors_for_known_stages` pass.
