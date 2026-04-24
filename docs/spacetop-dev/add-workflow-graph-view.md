---
id: 007
title: Render the workflow stage graph on the main TUI page
status: plan
source: captain feedback after build-initial-tui-overview
started: 2026-04-24T16:09:42Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

The main TUI page should visually present the workflow's stage graph — nodes for each stage with its defaults/properties (initial, terminal, gate, worktree, feedback-to), forward edges between stages, and a distinct edge style for feedback loops (e.g., `review --feedback-to--> implement`). The graph is derived from the parsed `WorkflowSnapshot`, not hard-coded, so it reflects whatever workflow is loaded.

## Problem statement

Today the main page shows a "Workflow" pane whose only workflow-shape information is a flat list of stage names with per-stage counts (see `src/ui/mod.rs::summary`). A reader cannot tell from that pane which stage is initial, which is terminal, which is a gate, which runs in a worktree, or that `review` loops back to `implement`. SpaceTop's value proposition is making Spacedock workflow state inspectable, and the stage topology is the single most load-bearing piece of that state. This task turns the summary pane into a true workflow graph that encodes those stage properties visually.

## Target user flow

1. User runs `spacetop` with a workflow directory (auto-discovered or via `-w/--workflow-dir`).
2. The overview renders with the workflow graph replacing the current summary pane, above the task list / preview split.
3. The user reads the graph left-to-right and sees, at a glance: the stage order, which stage is initial (`▶`) and terminal (`■`), which are gates (`⚑`), which run in a worktree (`⎇`), the `review ↶ implement` feedback arc, and per-stage item counts.
4. The task list and preview panes retain their existing behavior; selecting a task does not mutate the graph.
5. When a different workflow is loaded (via `-w` or auto-discovery), the graph re-renders to reflect the new stage topology.
6. On narrow terminals the graph collapses gracefully (see AC-3) so the rest of the TUI remains usable.

## Rendering approach (locked): horizontal stage ribbon

Picked: **single-row stage ribbon**, laid out left-to-right in `WorkflowDefinition::stages` declaration order, with a second row dedicated to feedback arcs, and a count row aligned under the node columns.

Why not Sugiyama / layered box-and-arrow:
- SpaceTop workflows are short and near-linear (typical: 3-7 stages, one or two feedback edges back to an earlier stage). A layered layout adds visual noise and wastes vertical terminal real estate for no information gain.
- Layered layouts need non-trivial redraw work. Task 008 (`auto-refresh-on-workflow-changes`) will recompute `WorkflowSnapshot` on every file-change event and redraw the frame; a ribbon is O(stages) to lay out and re-render every tick. We must stay on that budget.
- No external layout crate is taken on; the ribbon is a hand-rolled single pass over `definition.stages`.

Why not a pure text list (status quo): it does not encode initial/terminal/gate/worktree markers or feedback edges, which is the whole point of this task.

Layout sketch (real `docs/spacetop-dev` workflow, counts illustrative):

```
 ▶ design ──►  plan  ──► ⎇ implement ──► ⚑ review ──► ■ done
     2          1             1               0          3
                              ▲                │
                              └────────────────┘ ↶ feedback-to
```

The ribbon is drawn inside a `Block` titled `Workflow` (title augmented by task 006's scope indicator — see below).

### Node glyph encoding (locked)

Each stage node is rendered as `{leading_markers} {name} {trailing_markers}`:

| Stage property | Glyph | Placement |
|----------------|-------|-----------|
| `initial: true` | `▶` | leading |
| `terminal: true` | `■` | trailing |
| `gate: true` | `⚑` | leading, before `initial` marker if both |
| `worktree: true` | `⎇` | leading, after gate marker |
| `feedback_to: Some(target)` | `↶` arc on feedback row | edge, not node glyph |
| `fresh: true` | (not rendered; implementation detail of dispatch, not user-facing topology) | — |
| `concurrency` | (not rendered in graph; visible elsewhere) | — |

Marker ordering rule for nodes with multiple flags: `⚑ ⎇ ▶ name ■`. In practice only one or two markers apply to any single stage.

ASCII fallback set (for terminals that cannot render Unicode box-drawing / arrows): `>` initial, `#` terminal, `!` gate, `@` worktree, `<` feedback. Forward arrows render as `->` instead of `──►`. The implementation decides at render time based on a simple feature flag (default Unicode; ASCII when a `SPACETOP_ASCII=1` env var is set — leave room for auto-detect later, out of scope for this task).

### Feedback edges (locked)

Feedback edges are rendered as a curved arc on the row directly below the node row. For each stage whose `feedback_to` is set and whose target exists among the defined stages, draw a back-arc from the source node column to the target node column using `│`, `└`, `┘`, `◄` (or ASCII `|`, `+`, `<`). The arc is annotated once at the right edge with the literal text `↶ feedback-to` so the semantics are unambiguous. Multiple feedback arcs stack on additional rows if present, bounded by the feedback-row budget (see narrow-terminal rule).

### Per-stage counts overlay (locked)

Counts render on a dedicated row beneath each node, aligned so the count sits under the midpoint of its node's column. The count is the number of items whose `status` equals the stage name, taken from `App::stage_counts()`; no recomputation is added. The active stage (where the currently-selected task lives) gets a reversed-style cell so the user can see which node the selection sits on.

Counts row only — no badge inside the node glyph. Reasons: (1) node widths vary with stage name length and a badge breaks alignment; (2) a separate row keeps the node label unambiguously the stage name; (3) counts change often (on refresh) while node glyphs do not, and keeping them on separate rows keeps diffing simple.

## Placement on the main page (locked)

The graph **replaces** the current summary pane (the top `Constraint::Length(10)` block in `src/ui/mod.rs::render`). It does not toggle — it is always shown on the overview screen. The list/preview split below is unchanged.

Layout after this task:

```
┌ Workflow ─────────────────────────────────────────────────┐
│ {scope indicator from 006}  workflow: <path>              │  <- header row
│  ▶ design  ──► plan ──► ⎇ implement ──► ⚑ review ──► ■ done│  <- ribbon row
│     2          1            1               0           3  │  <- counts row
│                             ▲               │              │  <- feedback row(s)
│                             └───────────────┘ ↶ feedback-to│
└───────────────────────────────────────────────────────────┘
┌ Tasks ────────────────────┐┌ Preview ───────────────────────┐
│ ...                       ││ ...                            │
```

Height budget: `Constraint::Length(7)` (borders 2 + header 1 + ribbon 1 + counts 1 + feedback 1 + spare 1). Slightly smaller than today's 10-row summary, freeing terminal rows for the task list. When more than one feedback arc exists, the block grows by one row per extra arc up to a cap of two; further arcs collapse into the final-row annotation `+N more feedback edges`.

### Interaction with task 006 (archived view scope)

Task 006 introduces `ViewScope { Active, Archived }` on `App` and adds a scope indicator + archived count to the summary pane. Because this task replaces the summary pane, the design absorbs those surfaces:

- The header row of the new Workflow block carries the scope indicator (e.g., `[active]` / `[archived: 4]`) on the left and the workflow path on the right, matching where 006 would have placed them on the summary pane.
- The graph topology (nodes, edges, markers) is invariant across scopes — it is derived from `WorkflowDefinition`, which does not change with scope.
- The counts row reflects the current scope's item set: in `Active` scope counts come from non-archived items; in `Archived` scope they are pulled from archived items. Concretely, `App::stage_counts()` (or its scoped successor introduced by 006) is the single source of truth; the graph reads from it without caring which scope is active.
- No coupling to 006's `ViewScope` type name is required from this task. Task 006 lands first; this task consumes whatever `stage_counts()` returns plus whatever header fields 006 exposes. If 006's API changes mid-flight, the integration point is confined to the header row + counts row.

### Interaction with task 008 (auto-refresh)

Constraint: graph rendering must be cheap enough to redraw on every file-change refresh. The ribbon layout is O(stages) and does no allocation beyond the line buffers ratatui already produces per frame, so this is satisfied. No caching or dirty-tracking is required. The design deliberately avoids any layout step that scales with item count (e.g., per-item glyphs on the graph) for this reason.

## Parser / TUI constraints

What the parser already exposes (`src/domain/mod.rs`, verified against `src/parser.rs`):

- `WorkflowDefinition.stages: Vec<StageDefinition>` — declared order is preserved by the parser.
- `StageDefinition` fields used by the graph: `name`, `initial`, `terminal`, `gate`, `fresh`, `feedback_to: Option<String>`, `worktree`, `concurrency`. Every property the node-glyph table needs is present today.
- `App::stage_counts()` (`src/app.rs`) already produces `Vec<StageCount { name, items }>` in stage-declaration order; the graph consumes this directly.

**New domain types required: none.** The graph is a pure function of the existing `WorkflowSnapshot` plus the selection index (for active-stage highlighting) and (post-006) the view scope.

**Forward-edge inference:** the parser does not expose an explicit adjacency list. The graph treats forward edges as the implicit pairs `(stages[i], stages[i+1])` for all `i`, matching how Spacedock README-declared stage order drives dispatch. Feedback edges come from `feedback_to`. If a future workflow needs non-linear forward edges, that is a separate entity — this task keeps the sequential assumption explicit.

**Feedback target validation:** if a `feedback_to` value does not match any stage name, the graph omits that arc and does not error. The parser already accepts the field verbatim; validation elsewhere is out of scope.

**Code location:** new module `src/ui/graph.rs`, exposing `pub fn render_stage_graph(frame: &mut Frame<'_>, area: Rect, app: &App)`. `src/ui/mod.rs::render` replaces its `summary(app)` call with a call to `render_stage_graph` against the top `Constraint::Length(7)` area. The existing `summary` helper is removed. No changes to `src/app.rs` are required for this task (task 006 owns the scope field addition).

**Redraw cost:** O(stages) with small constants; re-renders on every tick / file-change event without guarding.

## Acceptance criteria

**AC-1 -- The main page renders each stage from the loaded workflow as a node in declaration order, with per-stage property markers encoded in the node glyph.**
Verified by: render test against `docs/spacetop-dev` fixture (backend `TestBackend::new(120, 20)`) that asserts the rendered buffer contains every stage name in declaration order and contains the initial marker glyph adjacent to `design`, the terminal marker glyph adjacent to `done`, the gate marker adjacent to `review`, and the worktree marker adjacent to `implement`.

**AC-2 -- The graph renders at least one feedback edge when the workflow declares `feedback-to`, and the edge is visually distinguished from forward edges.**
Verified by: same fixture render test asserts the rendered buffer contains the feedback annotation token (`↶ feedback-to` or ASCII fallback) and that the source/target of that arc align with `review` and `implement` columns.

**AC-3 -- The graph view reflects a different workflow topology when a different workflow is loaded.**
Verified by: render test across two fixture workflows (`docs/spacetop-dev` and a minimal test-only fixture with a different stage set and no feedback edge) asserts the rendered node labels differ and the feedback row is absent for the second fixture.

**AC-4 -- Graph rendering degrades gracefully when the terminal is too narrow to fit the full ribbon.**
Verified by: render test with `TestBackend::new(40, 20)` asserts the render does not panic, produces a compact single-line textual summary of stage names with counts (e.g., `design(2) → plan(1) → implement(1) → review(0) → done(3)`), and still contains every stage name in declaration order. A second test at `TestBackend::new(24, 20)` asserts a stacked one-stage-per-line fallback that still contains every stage name.

**AC-5 -- Per-stage counts render aligned beneath the corresponding node and reflect the App's `stage_counts()` output.**
Verified by: render test asserts each count string from `App::stage_counts()` is present in the rendered buffer within the counts row, and that (for the fixture) the count associated with the stage containing the initial selection is rendered with the active-style marker (checked via buffer cell style or a reversed-attribute assertion).

**AC-6 -- The Workflow block header carries the scope indicator surface that task 006 writes to, so the archived-scope integration does not regress when 006 lands.**
Verified by: render test constructs an App state with a stubbed scope label (via the same path 006 will use — e.g., a `scope_label()` accessor that returns `"active"` by default) and asserts the rendered Workflow block title or header row contains that label. If 006 has not yet landed, the test uses the default label; when 006 lands it will extend the label and the test continues to pass.

**AC-7 -- The graph is rendered by a dedicated `src/ui/graph.rs` module invoked from `src/ui/mod.rs`, and the previous `summary` helper is removed.**
Verified by: `grep -n 'fn render_stage_graph' src/ui/graph.rs` finds the function; `grep -n 'fn summary' src/ui/mod.rs` returns nothing; `src/ui/mod.rs::render` is updated to call `render_stage_graph` for the top area.

## Out of scope

- Toggling the graph on/off via a key binding (the graph is always visible on the overview screen in this task).
- Non-linear forward-edge inference (e.g., multi-child stage graphs); current Spacedock workflows are linear and this task encodes that assumption.
- Animation, color theming beyond ratatui defaults, or per-stage tooltips.
- Automatic ASCII/Unicode detection based on terminal capabilities (Unicode is default; `SPACETOP_ASCII=1` opts in to ASCII for test portability).
- Rendering the `fresh` or `concurrency` stage fields visually in the graph.

## Stage Report: design

- DONE: Problem statement and user flow are locked: rendering approach picked (ASCII/Unicode graph vs. stage ribbon vs. layered layout), node glyph encoding for initial/terminal/gate/worktree/feedback-to, and placement on the main page (with interaction notes against the summary/list/preview panes).
  Entity body now contains a locked "Rendering approach" section (horizontal stage ribbon), a "Node glyph encoding" table, a "Placement" section that explicitly replaces the summary pane, and interaction subsections for tasks 006 and 008.
- DONE: Acceptance criteria replace the placeholder section with concrete, verifiable AC-N bullets covering: graph reflects loaded workflow, feedback edges render, narrow-terminal degradation, per-stage counts, interaction with the 006 scope indicator.
  Acceptance criteria section rewritten with AC-1 through AC-7, each naming a specific render-test assertion or grep check.
- DONE: Parser/TUI constraints are named — what the parser already exposes on stages, any new domain types the graph needs, and where the rendering code lives (new `src/ui/graph.rs`? module inside `src/ui/`?).
  "Parser / TUI constraints" section enumerates the `StageDefinition` fields in use today, states no new domain types are required, and locks the render code at `src/ui/graph.rs` with `render_stage_graph`.

### Summary

Locked the workflow-graph view as a horizontal stage ribbon that replaces the existing summary pane, with Unicode-default / ASCII-fallback glyphs encoding initial / terminal / gate / worktree on the node and a dedicated feedback-arc row for `feedback-to` edges. Per-stage counts render on a row aligned under each node column, pulling from the existing `App::stage_counts()` so task 008's auto-refresh redraw stays O(stages). No new domain types are needed; all required fields are already on `StageDefinition`, and the graph integrates with task 006 via the block's header row + counts row without coupling to 006's type names.
