---
id: 007
title: Render the workflow stage graph on the main TUI page
status: implement
source: captain feedback after build-initial-tui-overview
started: 2026-04-24T16:09:42Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-add-workflow-graph-view
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

## Implementation plan

### File / module ownership

- **New: `src/ui/graph.rs`** — owns all ribbon rendering logic. Public surface is a single entry point plus helpers kept private:
  - `pub fn render_stage_graph(frame: &mut Frame<'_>, area: Rect, app: &App)` — the sole entry. Reads `app.snapshot().definition.stages`, `app.stage_counts()`, `app.selected_item()`, `app.workflow_dir()`, and (when 006 lands) `app.scope_label()`. Chooses the width tier based on `area.width` and dispatches to one of three private renderers. Reads `SPACETOP_ASCII` via `std::env::var` once at function entry to pick the glyph set.
  - Private helpers: `fn pick_width_tier(area_width: u16, stages: &[StageDefinition]) -> WidthTier`, `fn layout_columns(stages: &[StageDefinition], glyphs: &GlyphSet) -> Vec<ColumnLayout>`, `fn render_wide(...)`, `fn render_narrow(...)`, `fn render_very_narrow(...)`, `fn render_feedback_rows(...)`, and `fn glyphs_for(ascii: bool) -> GlyphSet`.
  - Private types (inside `graph.rs` only, not re-exported): `enum WidthTier { Wide, Narrow, VeryNarrow }`, `struct GlyphSet { initial, terminal, gate, worktree, feedback, forward_arrow, arc_vert, arc_left, arc_right }`, `struct ColumnLayout { node_text: String, column_center: u16, count: usize }`.
  - No `pub mod` re-exports from the module; only `render_stage_graph` is visible to `ui::mod`.

- **Modified: `src/ui/mod.rs`** — becomes a thin composer:
  - Add `mod graph;` and `use graph::render_stage_graph;`.
  - In `render`, change the top-area constraint from `Constraint::Length(10)` to `Constraint::Length(7)` (per locked layout), then call `render_stage_graph(frame, summary_area, app)` in place of `frame.render_widget(summary(app), summary_area)`.
  - Delete the `summary` helper entirely (AC-7 requires `grep 'fn summary' src/ui/mod.rs` to return nothing).
  - Keep `task_list` and `preview` unchanged.
  - Update the existing `renders_real_workflow_summary_task_list_and_preview` test: relax its `contains(&stage_line)` assertion (which depends on the `design: 2` textual form that no longer appears) to instead assert each stage name is present, since stage counts now live on a separate row. Move the detailed stage-count alignment and glyph assertions into the new `graph.rs` test module.

- **Not modified in this task: `src/app.rs`** — the graph pulls only from existing `App` accessors. Task 006 owns adding `scope_label()` (or equivalent). If 006 has not landed yet, the header row shows `active` as a hard-coded default string inside `render_stage_graph`, guarded behind a `// TODO(task-006): replace with app.scope_label()` comment so task 006's worker only touches `src/app.rs` and a single header-row line in `graph.rs`. No `pub` surface in `App` is added or removed by this task.

- **Not modified in this task: `src/domain/mod.rs`, `src/parser.rs`** — entity explicitly states no new domain types are required.

### Step-by-step

1. Add `mod graph;` to `src/ui/mod.rs` and create an empty `src/ui/graph.rs` exposing a stub `pub fn render_stage_graph(frame: &mut Frame<'_>, area: Rect, app: &App)` that delegates to the old `summary` path inside a `#[cfg(test)] use` bridge. Confirm `cargo check` passes.
2. In `graph.rs`, implement `glyphs_for(ascii: bool) -> GlyphSet` with both glyph sets from the entity (Unicode: `⚑ ⎇ ▶ ■ ↶ ──► │ └ ┘`; ASCII: `! @ > # < -> | + +`). Add unit test `glyphs_for_respects_ascii_flag`.
3. Implement `layout_columns` — for each `StageDefinition`, build `node_text` as `{leading_markers}{space}{name}{trailing_markers}` following the locked ordering rule `⚑ ⎇ ▶ name ■`. Separators are `forward_arrow` padded with single spaces. Returns column metadata including each node's center column (for aligning counts and feedback arc endpoints). Unit tests: `layout_columns_places_initial_marker_on_first_stage`, `layout_columns_places_terminal_marker_on_last_stage`, `layout_columns_marker_ordering_is_gate_worktree_initial_name_terminal`.
4. Implement `pick_width_tier`:
   - `Wide` if the assembled ribbon string fits inside `area.width - 2` (border budget).
   - `Narrow` if the compact `name(count) -> name(count) -> ...` form fits on one line inside `area.width - 2` but the wide form does not.
   - `VeryNarrow` otherwise — one stage per line.
   Unit test: `pick_width_tier_returns_expected_tier_for_sample_widths` exercising 120, 40, and 24 column widths against the `docs/spacetop-dev` stage set.
5. Implement `render_wide`: write four lines into the block body using `Paragraph::new(Vec<Line>)` inside a `Block::default().title(title_line).borders(Borders::ALL)`. Title line carries the scope label on the left and `workflow: <path>` on the right (ratatui `Line::from(vec![Span, Span])` with right-alignment style on the second span; if right-alignment is awkward, fall back to title = `"Workflow — [active] — <path>"` single-span form; the test asserts presence of each token, not placement). Lines: ribbon, counts, feedback arcs (conditional). Active-stage count cell uses `Style::default().add_modifier(Modifier::REVERSED)`.
6. Implement `render_feedback_rows`: for each stage with `feedback_to == Some(target)` where `target` matches a stage name, emit one line containing `│` at the source column center, `└` at the leftmost endpoint, `┘` at the rightmost endpoint, horizontal `─` fills between, and the annotation `↶ feedback-to` appended at the rightmost unused position (truncated if the terminal is tight). Cap at two feedback rows; overflow becomes a final text line `+N more feedback edges`. Unit test: `render_feedback_rows_draws_arc_between_source_and_target_columns` using a stubbed pair of `review -> implement` columns.
7. Implement `render_narrow`: single-line `design(2) → plan(1) → ⎇ implement(1) → ⚑ review(0) → ■ done(3)` textual summary. Keep per-stage markers inline as a prefix where present. No feedback arc in this tier; append `↶ feedback-to: review→implement` on the counts line if space remains.
8. Implement `render_very_narrow`: stacked one-stage-per-line form `{marker} {name} ({count})`, one line per stage. If the stack overflows vertical area, truncate and append `+N more`.
9. Wire `src/ui/mod.rs::render` to call `render_stage_graph` for the top area; drop `summary`; reduce the top constraint to `Constraint::Length(7)`.
10. Update the existing integration test in `src/ui/mod.rs` per the ownership note above, then add the render-test suite described below inside `src/ui/graph.rs`.
11. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

### Width-tier fallback strategy

| Tier | Trigger | Output |
|------|---------|--------|
| `Wide` | full ribbon (`Σ node_widths + (N-1) × 5`) fits in `area.width - 2` | header + ribbon row + counts row + feedback row(s) |
| `Narrow` | wide form overflows but compact `name(count) → ...` fits | header + single compact line (no feedback arc; textual feedback annotation instead) |
| `VeryNarrow` | narrow form overflows | header + stacked one-stage-per-line list |

Tier is recomputed each frame from `area.width`; no state carried across frames. O(stages) work per tier.

### Verification commands

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p spacetop --lib ui::graph` (new module's tests)
- `cargo test -p spacetop` (full suite — proves no regression in existing `ui::tests::renders_real_workflow_summary_task_list_and_preview` and `app::tests`)
- `grep -n 'fn render_stage_graph' src/ui/graph.rs` (must match)
- `grep -n 'fn summary' src/ui/mod.rs` (must return nothing)

## Test strategy

All tests live in `#[cfg(test)] mod tests` inside `src/ui/graph.rs`, plus one touch-up in `src/ui/mod.rs::tests`. Each uses `ratatui::backend::TestBackend` and a `buffer_text` helper identical to the existing one. Fixture workflow: `docs/spacetop-dev` (provides all four marker kinds and a `review -> implement` feedback edge). Second fixture: an in-test `WorkflowSnapshot` built via `App::from_snapshot` with three plain stages `alpha -> beta -> gamma` and no feedback edges.

Named tests:

1. `renders_wide_ribbon_with_unicode_glyphs_for_real_workflow` — `TestBackend::new(120, 20)`, real `docs/spacetop-dev`. Asserts: buffer contains each stage name in declaration order (via `find` indices being monotonic); `▶ design` substring; `■` near `done`; `⚑ review` substring; `⎇ implement` substring. Covers AC-1.
2. `renders_feedback_arc_with_annotation` — same fixture/width. Asserts buffer contains `↶ feedback-to`. Asserts the column index of `review` in the ribbon row equals the column index of the `│` cell in the feedback row (source alignment), and column index of `implement` equals the `└` endpoint column (target alignment). Covers AC-2.
3. `reflects_different_workflow_topology` — build the alpha/beta/gamma in-memory snapshot via `App::from_snapshot`. Render at `TestBackend::new(120, 20)`. Assert buffer contains `alpha`, `beta`, `gamma`; does NOT contain any of `design`, `review`, or `↶ feedback-to`. Covers AC-3.
4. `narrow_tier_renders_compact_textual_summary` — `TestBackend::new(40, 20)`. Assert buffer contains every stage name in declaration order, contains at least one `→` (or `->` in ASCII mode), and does NOT panic. Contains each stage count substring from `stage_counts()`. Covers AC-4 (narrow tier).
5. `very_narrow_tier_stacks_one_stage_per_line` — `TestBackend::new(24, 20)`. Assert buffer contains every stage name on separate lines (each name's row index is strictly increasing). Covers AC-4 (very-narrow tier).
6. `counts_row_aligns_under_nodes_and_marks_active_stage` — `TestBackend::new(120, 20)` with the real fixture. For each stage, assert the stage's count string appears directly under the stage name column (column center ± stage-name half-width). Assert the count cell for the stage containing `App::selected_item()`'s status has `Modifier::REVERSED` set (inspected via `buffer.cell((x, y)).style()`). Covers AC-5.
7. `header_row_contains_scope_label_and_workflow_path` — real fixture, 120×20. Assert rendered title/header contains the default scope label `active` (or whatever `app.scope_label()` returns after 006 lands) and contains the workflow path string. Covers AC-6.
8. `ascii_fallback_swaps_glyphs_when_env_set` — set `SPACETOP_ASCII=1` via a scoped env guard helper (`fn with_env(key, val, f)` using `std::env::set_var`/`remove_var`; run serially via `#[serial_test]` or a local mutex to avoid cross-test races). Render real fixture at 120×20. Assert buffer contains `>` (initial), `#` (terminal), `!` (gate), `@` (worktree), `<` (feedback), `->` (forward). Assert buffer does NOT contain `▶`, `■`, `⚑`, `⎇`, `↶`, `──►`. Covers the ASCII fallback requirement.
9. `module_surface_is_minimal` — compile-time check: `let _: fn(&mut Frame<'_>, Rect, &App) = crate::ui::graph::render_stage_graph;` inside a `#[test]` confirming the public signature doesn't regress.

The existing `src/ui/mod.rs::tests::renders_real_workflow_summary_task_list_and_preview` is updated to drop its `{name}: {count}` substring assertion (that form no longer exists) and keep the selected-item, preview, and stage-name presence assertions.

## Stage Report: plan

- DONE: Step-by-step plan enumerates the files to add/change (`src/ui/graph.rs`, `src/ui/mod.rs`, possibly `src/app.rs` for scope indicator plumbing), the width-tier fallback strategy, and verification commands.
  See "Implementation plan" section above — 11 numbered steps, width-tier table, and verification command list.
- DONE: Test strategy names specific render tests: wide-terminal unicode render, narrow-tier degraded render, very-narrow textual summary, feedback arc present, counts row aligned, `SPACETOP_ASCII=1` fallback.
  See "Test strategy" section — nine named tests explicitly covering each AC; ASCII fallback is test 8; narrow and very-narrow are tests 4 and 5; feedback arc is test 2; counts alignment is test 6.
- DONE: File/module ownership — explicit map of who owns the graph rendering (new module), what stays in `src/ui/mod.rs`, and what the interface looks like (`fn render_graph(frame, area, snapshot, counts, scope_indicator)` or similar).
  See "File / module ownership" subsection — `graph.rs` owns everything via `pub fn render_stage_graph(frame: &mut Frame<'_>, area: Rect, app: &App)`; `ui/mod.rs` keeps `task_list`/`preview` and the top-level `render` composer only; `src/app.rs` is untouched by this task and left to task 006.

### Summary

Produced a step-by-step implementation plan that confines new rendering logic to `src/ui/graph.rs` behind a single `render_stage_graph(frame, area, &App)` entry point, leaves `src/app.rs` to task 006, and shrinks the top pane to `Constraint::Length(7)`. Width-tier fallback is a three-way split (`Wide` / `Narrow` / `VeryNarrow`) recomputed each frame from `area.width` so task 008's auto-refresh stays O(stages). Test strategy names nine render tests in `graph.rs` that each map to a specific acceptance criterion, including a `SPACETOP_ASCII=1` guard for the ASCII fallback and a column-alignment assertion for both the counts row and the feedback arc endpoints.
