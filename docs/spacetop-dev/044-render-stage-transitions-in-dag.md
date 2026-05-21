---
id: 044
title: Render stage transitions in the DAG (terminal stages disconnected from predecessors)
status: plan
source: captain
started: 2026-05-21T08:29:26Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

The Spacetop graph view lays stages out left-to-right in the order they appear under `stages.states`, drawing a single chain of `──▶` edges between adjacent entries. It ignores the `stages.transitions` block when the workflow README declares one. On any workflow with branching transitions and multiple terminal stages, the DAG misrepresents the real flow.

Reproduction: open `/Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/` in Spacetop. That workflow declares 12 stages including 4 terminal ones (`expanded`, `ideated`, `done`, `rejected`) and an explicit `stages.transitions:` list. The rendered DAG strings all 12 stages into one chain (`pending → scoping → ideate → review → smoke → run → analyze → promote → expanded → ideated → done → rejected`). The terminal stages appear as if they followed `promote` sequentially, when in reality each terminal stage has a specific predecessor:

- `expanded` is reached from `scoping`
- `ideated` is reached from `ideate`
- `done` is reached from `promote`
- `rejected` is reached from `review`, `smoke`, and `analyze`

So the captain sees four terminal nodes hanging off the wrong place in the chain, with no edges drawn to their real predecessors.

Root cause sketch: `src/domain/` models `StageDefinition` (initial, terminal, gate, worktree, feedback-to, etc.) but does not parse `stages.transitions`. `src/ui/graph.rs` builds chain rows directly from the stage slice in declaration order; only `feedback-to` arcs are drawn as out-of-band cross-row edges. The forward edges between non-adjacent stages declared in `transitions:` have nowhere to come from.

## Acceptance criteria

Each AC names a property of the finished entity (not a stage action) and how it is verified.

**AC-1 — Transitions are parsed into the domain model.**
The README's `stages.transitions` list (each item with `from`, `to`, optional `label`) is parsed and exposed on `WorkflowDefinition` (or equivalent). Workflows without a `transitions:` block continue to behave as today (implicit linear chain from `states:` order).
Verified by: a unit test in `src/domain/` (or `src/parser.rs`) that loads a YAML fixture containing a `transitions:` block and asserts the parsed edges match the declared `from`/`to` pairs.

**AC-2 — Terminal stages are connected to their real predecessors in the DAG.**
When a workflow declares `transitions`, the graph view draws an edge from each declared `from` stage to its `to` stage. Terminal stages that are reached from a non-adjacent predecessor (`scoping → expanded`, `ideate → ideated`, `promote → done`) are visibly linked to that predecessor instead of being chained after an unrelated stage.
Verified by: a snapshot or assertion test in `src/ui/graph.rs` that renders the dataagentbench research workflow definition and confirms each of the four terminal stages has an inbound edge from its declared predecessor(s).

**AC-3 — Multi-source terminal stages show all incoming edges.**
The `rejected` terminal stage in the research workflow has three declared predecessors (`review`, `smoke`, `analyze`). All three are represented as inbound edges to `rejected` in the rendered DAG.
Verified by: the same test as AC-2 (or a sibling test) asserts `rejected` has three inbound edges, one per declared source.

**AC-4 — Existing single-chain workflows are unaffected.**
Workflows that omit `stages.transitions` (e.g., this `spacetop-dev` workflow, `recce-team` workflows without explicit transitions) continue to render as today — a single left-to-right chain in `states:` order, with feedback-to arcs unchanged.
Verified by: existing graph tests pass without modification, plus an explicit regression test using a fixture with no `transitions:` block.

**AC-5 — `make lint` is clean.**
`make lint` (clippy `-D warnings`) passes after the change. No new `#[allow(...)]` introduced without justification.
Verified by: running `make lint` locally.
