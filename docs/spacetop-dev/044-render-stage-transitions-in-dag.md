---
id: 044
title: Render stage transitions in the DAG (terminal stages disconnected from predecessors)
status: review
source: captain
started: 2026-05-21T08:29:26Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-044-render-stage-transitions-in-dag
issue:
pr:
mod-block: merge:pr-merge
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

## Implementation Plan

This plan splits the work into three independently testable units. Units 1 and 2 are headless (no terminal, no `ratatui::TestBackend`); unit 3 drives the existing `TestBackend` harness in `src/ui/graph/tests.rs`. Each unit ships its own tests and can be committed independently.

### Design summary

- Add a first-class `StageTransition { from, to, label }` to the domain model and parse `stages.transitions` from the README frontmatter.
- Treat the parsed `transitions` as the authoritative edge set when present. When the block is absent, **synthesize** the implicit linear chain from `stages.states` order so the rest of the code (renderer included) only ever looks at one edge list. This is the AC-4 fallback — it preserves today's behavior bit-for-bit on workflows like `docs/spacetop-dev/` that omit `transitions:`.
- In the graph renderer, replace the implicit "edge between adjacent columns" assumption with an explicit edge-aware draw step: forward `──▶` for edges between same-row neighbors, and a non-adjacent edge style (a labeled tail glyph or arc, design locked in unit 3) for edges that skip columns or cross rows. Terminal stages that previously appeared in a chain after `promote` will instead receive an inbound non-adjacent edge from their declared predecessor(s) — including the multi-source case for `rejected`.
- `feedback-to` arcs remain a separate channel rendered UNDER the chain row exactly as today; they are out-of-band, not part of `transitions:`.

### Unit 1 — Parse `stages.transitions` into the domain model

**Scope:** `src/domain/mod.rs`, `src/parser/readme.rs`. Pure data — no rendering.

**Files to touch:**
- `src/domain/mod.rs`: add `pub struct StageTransition { pub from: String, pub to: String, pub label: Option<String> }`; add `pub transitions: Vec<StageTransition>` to `WorkflowDefinition`. Document the no-block fallback (empty vec, NOT a synthesized chain — see unit 2 for why the synthesis happens at the consumer boundary, not in the field).
- `src/parser/readme.rs`: add `transitions: Option<Vec<RawTransition>>` to `RawStageBlock`; define `RawTransition { from, to, label }` (kebab-case keys via `#[serde(rename = "from"/"to"/"label")]` — though all three are already kebab-safe, keep it explicit). Map the raw rows into `StageTransition` and populate `WorkflowDefinition.transitions`. When the block is absent, leave the vec empty.
- Construct-sites for `WorkflowDefinition` in tests need the new field. Sweep: `src/ui/graph/tests.rs` (several literal `WorkflowDefinition { ... }` constructions visible in lines 30–46, 207–223, 254–273), `src/app/tests.rs` (search for `WorkflowDefinition {`), and any other in-tree fixture. Add `transitions: Vec::new()` to each.

**Tests (all in `src/parser/readme.rs` `#[cfg(test)] mod tests`):**
- `transitions_block_is_parsed_into_definition`: feed an inline YAML fixture mirroring a slim subset of `dataagentbench/docs/research/README.md` — at minimum `pending → scoping`, `scoping → expanded`, `review → rejected`, `smoke → rejected`, `analyze → rejected`, `promote → done` — and assert `wf.transitions` contains exactly those edges with `from`/`to`/`label` matching the YAML.
- `missing_transitions_block_leaves_empty_vec`: fixture with only `stages.states` (no `transitions:` key). Assert `wf.transitions.is_empty()`. This is the AC-4 parser-level guarantee.
- `transitions_block_with_labels_round_trips`: assert one row's `label` is `Some(...)` and one row's `label` is `None` for an entry that omits the `label:` key.
- `parse_workflow_readme_real_dataagentbench_research_workflow_fixture`: copy the dataagentbench `stages:` block (frontmatter only — no need to vendor the entire README) into a fixture under `tests/fixtures/transitions_research.md` and assert all 13 declared transitions are present and the four terminal stages (`expanded`, `ideated`, `done`, `rejected`) have inbound edges from `scoping`, `ideate`, `promote`, and (`review`, `smoke`, `analyze`) respectively.

**Verification command:** `cargo test --lib parser::readme`

### Unit 2 — Edge resolution helper on `WorkflowDefinition`

**Scope:** `src/domain/mod.rs`. Still pure — no rendering, no terminal.

**Why a separate unit:** the renderer needs to ask "what edges does this workflow have?" and get the same answer whether `transitions:` was declared or not. Putting the no-transitions fallback in a single helper means unit 3 only consumes `effective_transitions()` and never touches the synthesized-chain branch directly.

**Add to `WorkflowDefinition`:**
- `pub fn effective_transitions(&self) -> Vec<StageTransition>`:
  - If `self.transitions` is non-empty, clone and return it verbatim.
  - Otherwise, synthesize `n - 1` transitions from `self.stages` in declaration order: `(stages[0] → stages[1], stages[1] → stages[2], …)`, with `label: None`. This is the AC-4 regression contract: the renderer sees the same edge shape as today for workflows that omit `transitions:`.

**Tests (in `src/domain/mod.rs` `#[cfg(test)] mod tests`):**
- `effective_transitions_returns_declared_set_when_present`: build a `WorkflowDefinition` with 3 stages and 2 explicit `transitions` (skipping the middle stage); assert `effective_transitions()` returns those 2 entries verbatim (not the synthesized chain).
- `effective_transitions_synthesizes_linear_chain_when_absent`: build a `WorkflowDefinition` with 5 stages and an empty `transitions` vec; assert `effective_transitions()` returns the 4 implicit `stages[i] → stages[i+1]` edges, each with `label: None`.
- `effective_transitions_for_single_stage_is_empty`: 1-stage workflow with empty `transitions` returns `vec![]` (no synthesized edge).
- `effective_transitions_for_zero_stages_is_empty`: 0-stage workflow returns `vec![]`.

**Verification command:** `cargo test --lib domain::tests::effective_transitions`

### Unit 3 — Render non-adjacent edges in the DAG

**Scope:** `src/ui/graph.rs`, `src/ui/graph/tests.rs`. This is the only unit that touches the terminal renderer and the only one that owns the visual contract.

**Files to touch:**
- `src/ui/graph.rs`:
  - In `render_dag`, after the column layout (`dag_layout_columns`) and row plan (`dag_layout_rows`) are built, walk `definition.effective_transitions()` and bucket each edge:
    - **Same-row, adjacent:** `from` and `to` land on the same row at columns `i` and `i+1` (the row's `col_indices` are consecutive in the workflow's stage order). These keep today's inline `──▶` separator rendering — emit no extra geometry. (Implementation note: the current chain renderer assumes adjacency in `cols`, so this case is the "no change" path. The split happens at the bucket step, not in the inline arrow code.)
    - **Same-row, non-adjacent (skip):** `from` and `to` on the same row but with at least one column between them. Render a labeled overhead arc above the chain row — a new rendering channel analogous to the existing under-chain feedback-arc channel — using `╭ ─ ╮` corners + `↓` arrowhead. Reserve at most `MAX_NON_ADJACENT_ROWS = 2` per chain row to bound vertical growth, then fall through to the annotation tail (next bullet).
    - **Cross-row:** `from` and `to` on different chain rows. Reuse the existing `CrossRowFeedbackArc` annotation-tail pattern (`collect_cross_row_feedback_arcs` in lines 745–775) to emit a one-line `from → to` annotation. This is the same degradation strategy `feedback-to` arcs already use, so the visual budget stays predictable.
  - The chain row itself (`──▶` between adjacent same-row columns) keeps drawing today. The new arc channel is additive: it draws on top of (or above) the chain rows without changing them.
  - In `render_narrow` and `render_very_narrow`, do **not** attempt to draw arc geometry — these tiers already wrap aggressively and any cross-column arc is meaningless. Instead, append an annotation tail (`↳ from → to` style, distinct from the existing `↩ rollback on reject: ...` tail) for every declared transition that the implicit reading order does not already imply. This keeps the narrow tiers truthful about the topology even when the geometry is sacrificed.

**Tests (all in `src/ui/graph/tests.rs`):**
- `dag_renders_inbound_edge_for_non_adjacent_terminal_predecessor` (AC-2): build a `WorkflowSnapshot` with the dataagentbench research stages and transitions; render via the existing `render_to_string(&app, 200, 30)` harness; assert the rendered string contains an overhead arc glyph (`╭` / `╮` / `↓`) at the column ranges spanning `scoping → expanded`, `ideate → ideated`, and `promote → done`, and that none of those terminal stages are linked only by the adjacent `──▶` from the prior chain column.
- `dag_renders_three_inbound_edges_for_rejected` (AC-3): same fixture; assert the rendered string contains **three** distinct arc geometries (or three annotation-tail lines) for `review → rejected`, `smoke → rejected`, `analyze → rejected`. The assertion counts the number of `→ rejected` substrings in either the arc-label region or the annotation tail.
- `dag_omits_arcs_when_no_transitions_block` (AC-4): build a `WorkflowSnapshot` with 5 stages and `transitions: Vec::new()`; render at the same `(width, height)` as the existing `renders_wide_ribbon_with_unicode_glyphs_for_real_workflow` test; assert the rendered string is byte-equal (or at least: contains no `╭`/`╮` overhead arc glyphs beyond the existing inter-row connectors, and the chain row contains the same `──▶` count as before). This is the explicit regression test AC-4 calls for.
- `dag_real_spacetop_dev_workflow_unchanged` (AC-4 belt-and-braces): re-run the existing `renders_wide_ribbon_with_unicode_glyphs_for_real_workflow` assertions; they must keep passing without modification because `docs/spacetop-dev/README.md` declares no `transitions:` block — its `effective_transitions()` synthesizes the same `design → plan → implement → review → done` chain the renderer sees today.

**Fixture mirroring the dataagentbench bug case:** add `src/ui/graph/tests.rs` helper `fn research_workflow_definition() -> WorkflowDefinition` that constructs the 12-stage states + 13 declared transitions verbatim from the dataagentbench README's frontmatter. Both AC-2 and AC-3 tests share this helper so the bug case is exercised by a single source of truth.

**Verification commands:**
- `cargo test --lib ui::graph`
- `cargo test` (full suite)
- `make lint` (AC-5)

### Sequencing

Unit 1 → Unit 2 → Unit 3 in that order. Unit 1 lands the data; Unit 2 lands the helper that gives every consumer the same edge view; Unit 3 lands the renderer and is the only unit that can be visually verified against the dataagentbench reproduction. Each unit's test target (`cargo test --lib parser::readme`, `domain::tests::effective_transitions`, `ui::graph`) runs in isolation so commits stay small and bisect-friendly. `make lint` must be clean at every commit, not just the last.

### Risks and mitigations

- **WorkflowDefinition field churn breaks test fixtures.** Mitigation: explicit sweep listed in Unit 1's "Files to touch" — add `transitions: Vec::new()` to every literal construction; `cargo build --tests` after the field add catches anything missed.
- **Arc geometry overflows the chain pane.** Mitigation: `MAX_NON_ADJACENT_ROWS = 2` cap mirrors the existing `MAX_FEEDBACK_ROWS = 2` budget; everything beyond falls through to the annotation tail. The DAG-vs-narrow tier picker (`pick_width_tier`) already accounts for arc rows in `dag_total_line_count`; extend that accounting to non-adjacent arc rows so the tier fallback stays correct when many transitions are declared.
- **Narrow / VeryNarrow tier annotation noise.** Mitigation: only annotate transitions whose `(from → to)` is NOT implied by states-order adjacency. Adjacent same-order transitions stay implicit so the tail does not flood with redundant `pending → scoping`-style noise.

## Stage Report: plan

- DONE: Plan separates the domain/parser change (modeling stages.transitions) from the graph rendering change (drawing non-adjacent edges) into testable units that can each ship behind unit tests without driving the TUI.
  Units 1 (parser/domain field) and 2 (`effective_transitions()` helper) are headless; Unit 3 is the only one that drives `TestBackend`. Each has its own `cargo test` target.
- DONE: Plan names the concrete files to touch (e.g., src/domain/mod.rs, src/parser.rs, src/ui/graph.rs) and, per AC, the specific test fixture or assertion that proves each one — including a fixture mirroring dataagentbench/docs/research transitions.
  Files listed per unit; shared `research_workflow_definition()` fixture mirrors the dataagentbench 12-state + 13-transition block and is consumed by the AC-2 and AC-3 tests.
- DONE: Plan documents the no-transitions fallback so workflows without a transitions: block keep rendering as a left-to-right chain (AC-4 regression path).
  `effective_transitions()` synthesizes the implicit `stages[i] → stages[i+1]` chain when the parsed vec is empty; AC-4 is enforced by `dag_omits_arcs_when_no_transitions_block` plus the existing `renders_wide_ribbon_with_unicode_glyphs_for_real_workflow` continuing to pass unchanged on `docs/spacetop-dev/`.

### Summary

The plan is three units: (1) parse `stages.transitions` into a new `StageTransition` field on `WorkflowDefinition` with tests in `src/parser/readme.rs`; (2) add an `effective_transitions()` helper on `WorkflowDefinition` that synthesizes the implicit linear chain when no transitions block is declared, locking the AC-4 fallback into a single chokepoint; (3) extend `src/ui/graph.rs` `render_dag` to bucket edges as adjacent / non-adjacent-same-row / cross-row, drawing an overhead arc channel for the non-adjacent case and reusing the existing annotation-tail degradation for cross-row, with a shared dataagentbench fixture driving the AC-2 and AC-3 tests. Each unit is independently committable; `make lint` is required clean at every step.

## Stage Report: implement

- DONE: Domain + parser changes for stages.transitions land with passing unit tests against both a transitions-fixture (mirroring dataagentbench/docs/research) and a no-transitions fixture; effective_transitions() returns the implicit linear chain when no block is declared (AC-1 + AC-4 chokepoint).
  Commit 931afa0; 4 domain + 4 parser tests passing including `parse_workflow_readme_research_fixture_terminal_edges` and `missing_transitions_block_leaves_empty_vec`.
- DONE: Graph rendering draws inbound edges to each terminal stage from its declared predecessor(s) for the research fixture — including all three sources (review, smoke, analyze) for rejected — verified by an assertion test in src/ui/graph.rs (AC-2 + AC-3).
  Commit 93acd91; `dag_renders_inbound_edge_for_non_adjacent_terminal_predecessor` (AC-2), `dag_renders_three_inbound_edges_for_rejected` (AC-3, exactly 3 `→ rejected` substrings), `dag_omits_arcs_when_no_transitions_block` (AC-4), and `collect_extra_transitions_for_research_fixture_lists_all_non_adjacent_edges` all green.
- DONE: make lint and cargo test are both green at end of implement; no new #[allow(...)] introduced without justification (AC-5).
  `make lint` clean (clippy -D warnings); cargo test reports 303 lib + 4 + 10 integration tests passing; `git diff main` shows no new `#[allow(...)]`.

### Summary

Implemented the three-unit plan: (1) added `StageTransition { from, to, label }` to the domain model and parsed `stages.transitions` from the README; (2) added `WorkflowDefinition::effective_transitions()` that synthesises the implicit linear chain when no block is declared, locking AC-4 into a single chokepoint; (3) extended `render_dag` (and the narrow tiers) to emit a one-line annotation tail for every declared edge that the inline chain rendering does not already draw. Rather than the originally-sketched overhead-arc geometry, the implementation reuses the existing cross-row degradation pattern — a single annotation line per non-adjacent edge — which is sufficient for AC-2/AC-3, keeps the change small, and avoids new geometry budget bookkeeping in the row planner.
