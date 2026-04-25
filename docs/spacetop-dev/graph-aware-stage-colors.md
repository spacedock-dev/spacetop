---
id: "013"
title: Assign stage colors using graph-aware coloring — no same color on adjacent stages
status: review
source: captain feedback 2026-04-25
started: 2026-04-25T10:37:39Z
completed:
verdict:
score: 0.8
worktree: .worktrees/spacedock-ensign-graph-aware-stage-colors
issue:
pr:
mod-block: merge:pr-merge
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

## Stage Report: plan

- DONE: Step-by-step plan naming exact data structures, functions, and files for the greedy graph coloring pass.
  See plan below.
- DONE: Test strategy: proposed assertions for AC-1 (adjacent differ), AC-2 (max 2 colors in linear 5-stage), AC-3 (familiar names preserved).
  See test strategy below.

### Implementation Plan

**Step 1 — Add `assign_stage_colors` free function in `src/ui/mod.rs`**

Signature:
```rust
pub(crate) fn assign_stage_colors(stages: &[StageDefinition]) -> Vec<Color>
```

Algorithm:
1. Build an undirected adjacency set: for each stage index `i`, add neighbor `i+1`; if `stages[i].feedback_to` names stage `j`, add neighbors `i` and `j` to each other's sets.
2. Iterate stages in definition order. For each stage `i`:
   a. Look up `preferred_color(stages[i].name)` using the existing `stage_color` name-based mapping (without the hash fallback — return `None` for unknown names).
   b. Collect the set of colors already assigned to neighbors.
   c. If the preferred color is not in the neighbor-colors set, assign it.
   d. Otherwise, walk `PALETTE` in order and assign the first color not in the neighbor-colors set.
3. Return `Vec<Color>` (same length as `stages`).

Palette constant (in `src/ui/mod.rs`):
```rust
const GRAPH_PALETTE: &[Color] = &[
    Color::Blue, Color::Cyan, Color::Yellow, Color::Magenta,
    Color::Green, Color::LightBlue, Color::LightMagenta, Color::Red,
];
```

**Step 2 — Add `stage_colors: Vec<Color>` field to `WorkflowDefinition` in `src/domain/mod.rs`**

```rust
pub struct WorkflowDefinition {
    // ... existing fields ...
    pub stage_colors: Vec<Color>,
}
```

`ratatui::style::Color` must be imported here. Add `use ratatui::style::Color;` to `src/domain/mod.rs`.

Default value for `stage_colors` is `Vec::new()`. All existing `WorkflowDefinition` construction sites in tests that use struct literal syntax must add `stage_colors: Vec::new()`.

**Step 3 — Populate `stage_colors` during `WorkflowSnapshot` construction in `src/parser.rs`**

In `load_workflow_dir`, after `definition` is built from `parse_workflow_readme`, call:
```rust
definition.stage_colors = crate::ui::assign_stage_colors(&definition.stages);
```

Note: `assign_stage_colors` lives in `src/ui/mod.rs` but is `pub(crate)`. Since `parser.rs` is in the same crate, this is accessible. Alternatively, move the coloring function to `src/domain/mod.rs` (keeping colors out of the parser) — either location is acceptable. The simpler choice is to call it in `load_workflow_dir` right after stage list is finalized, and have `assign_stage_colors` remain in `src/ui/mod.rs`.

Also call it in `OverviewState::from_snapshot` is not needed — the snapshot already carries the colors. However, for the synthetic test-only snapshots that bypass `load_workflow_dir`, a helper is needed. The cleanest approach: `WorkflowSnapshot` gains a constructor method that populates colors; or test helpers call `assign_stage_colors` directly.

**Step 4 — Add `stage_color_for` lookup method on `WorkflowDefinition`**

```rust
impl WorkflowDefinition {
    pub fn stage_color_for(&self, stage_name: &str) -> Color {
        self.stages
            .iter()
            .position(|s| s.name == stage_name)
            .and_then(|i| self.stage_colors.get(i).copied())
            .unwrap_or_else(|| stage_color(stage_name))  // fallback to name-based
    }
}
```

This keeps the existing `stage_color` name-based function as a last-resort for archived items whose stage name may not be in the current workflow definition.

**Step 5 — Update call sites in `src/ui/mod.rs` and `src/ui/graph.rs`**

`src/ui/mod.rs`:
- `build_task_list_items`: replace `stage_color(&item.status)` with `state.snapshot().definition.stage_color_for(&item.status)`.
- `build_preview_header_lines`: same replacement for the `status_color` binding.

`src/ui/graph.rs` (`render_wide`):
- Replace `stage_color(&col.stage_name)` with a lookup into a pre-built `&[Color]` slice passed from `render_stage_graph`. `render_stage_graph` reads `state.snapshot().definition.stage_colors` and passes it into `render_wide`.

**Step 6 — Update tests**

All `WorkflowDefinition` struct-literal construction sites in tests need `stage_colors: Vec::new()` (or a computed value). Grep target: `WorkflowDefinition {` in `src/app.rs`, `src/ui/mod.rs`, `src/ui/graph.rs`, `src/parser.rs`.

Existing test `stage_color_assigns_distinct_colors_for_known_stages` tests the name-based fallback; it must continue to pass.

Existing test `graph_ribbon_uses_stage_colors_per_stage` tests that at least 3 stage colors appear in the render; it will pass as long as the graph-aware assignment still emits distinct colors for the 5-stage workflow.

### Test Strategy

**AC-1 — adjacent stages get different colors**

Module: `src/ui/mod.rs` (or a new `src/ui/color.rs` test), test name: `graph_coloring_no_adjacent_same_color`

```rust
// 4-stage workflow: alpha → beta → gamma → delta, with gamma feedback_to: alpha
let stages = vec![
    stage("alpha", ...),
    stage("beta", ...),
    stage("gamma", feedback_to: Some("alpha"), ...),
    stage("delta", ...),
];
let colors = assign_stage_colors(&stages);
// Adjacent pairs: (0,1), (1,2), (2,3), (2,0) via feedback
assert_ne!(colors[0], colors[1]); // alpha vs beta
assert_ne!(colors[1], colors[2]); // beta vs gamma
assert_ne!(colors[2], colors[3]); // gamma vs delta
assert_ne!(colors[2], colors[0]); // gamma vs alpha (feedback edge)
```

**AC-2 — linear 5-stage workflow uses at most 2 distinct colors**

Test name: `graph_coloring_linear_path_uses_at_most_two_colors`

```rust
// A path graph has max degree 2; greedy on a path alternates 2 colors.
let stages = vec![
    stage("a", ...), stage("b", ...), stage("c", ...),
    stage("d", ...), stage("e", ...),
];
// No feedback edges.
let colors = assign_stage_colors(&stages);
let distinct: std::collections::HashSet<Color> = colors.iter().copied().collect();
assert!(distinct.len() <= 2, "linear path needs at most 2 colors, got {}", distinct.len());
// Adjacency constraint still holds:
for i in 0..stages.len() - 1 {
    assert_ne!(colors[i], colors[i + 1]);
}
```

Note: AC-2 in the spec says "max 2 colors in linear 5-stage". The greedy algorithm on a path graph (alternating) indeed uses exactly 2 colors when no preferred colors conflict. The test asserts `<= 2` to be robust to edge cases.

**AC-3 — familiar names keep preferred colors**

Test name: `graph_coloring_preserves_preferred_colors_for_standard_workflow`

```rust
// Standard spacetop-dev 5-stage workflow (design→plan→implement→review→done,
// review feedback_to: implement).
let stages = vec![
    stage("design", initial: true, ...),
    stage("plan", ...),
    stage("implement", worktree: true, ...),
    stage("review", gate: true, feedback_to: Some("implement"), ...),
    stage("done", terminal: true, ...),
];
let colors = assign_stage_colors(&stages);
assert_eq!(colors[0], Color::Blue);    // design
assert_eq!(colors[1], Color::Cyan);    // plan
assert_eq!(colors[2], Color::Yellow);  // implement
assert_eq!(colors[3], Color::Magenta); // review
assert_eq!(colors[4], Color::Green);   // done
```

This test documents that the preferred-color hint mechanism preserves the familiar palette when there is no conflict.

### Module and File Ownership

| File | Change |
| --- | --- |
| `src/domain/mod.rs` | Add `stage_colors: Vec<Color>` to `WorkflowDefinition`; add `use ratatui::style::Color`; add `stage_color_for` method |
| `src/ui/mod.rs` | Add `assign_stage_colors` function; update `build_task_list_items` and `build_preview_header_lines` call sites; update test struct literals |
| `src/ui/graph.rs` | Pass colors slice into `render_wide`; update test struct literals |
| `src/parser.rs` | Call `assign_stage_colors` in `load_workflow_dir` to populate `definition.stage_colors`; update test struct literals |
| `src/app.rs` | Update `WorkflowDefinition` struct literal sites in test helpers |

### Summary

The plan identifies `assign_stage_colors(&[StageDefinition]) -> Vec<Color>` in `src/ui/mod.rs` as the core new function, `WorkflowDefinition.stage_colors: Vec<Color>` and `stage_color_for(&str) -> Color` as the domain additions, and `load_workflow_dir` in `src/parser.rs` as the population site. The test strategy provides concrete assertion patterns for all three acceptance criteria (AC-1 through AC-3) with exact stage fixture configurations. AC-4 regression coverage is provided by the existing test suite; no new tests are needed for it beyond verifying `cargo test` passes.
