---
title: Hide runtime activity from task list and detail header
status: implement
source: Captain request to remove unreliable per-task agent status, 2026-10-02
kind: feature
risk: low
milestone: v1-maintenance
proof: Ratatui TestBackend assertions for list and preview metadata across runtime states, scopes and terminal widths; workflow stages and durable gates remain visible.
started: 2026-10-02T02:11:23Z
completed:
verdict:
worktree: .worktrees/spacedock-ensign-hide-runtime-activity-in-task-views
issue:
pr:
mod-block:
id: 084
---

Make task browsing clearer by removing inferred agent runtime activity from the task list and detail header. The captain finds Idle unhelpful and real agent status difficult to determine for an individual task, and prefers those surfaces not to suggest a live activity state.

## Scope

- Remove runtime activity labels (Idle, Running and HumanGate, including Worker/FirstOfficer variants) and activity-only markers/colors from task-list rows.
- Remove the runtime/session/activity-status/updated attribution line from the detail/preview header so stale inference does not appear as current information.
- Keep workflow stage/status, score, source, worktree markers, archived verdicts and durable gate readiness/details visible; those are workflow facts, separate from inferred session activity.
- Touches: crates/spacetop/src/ui/list.rs, ui/preview.rs, nearby rendering tests and affected docs.
- Non-goals: Removing backend activity detection, changing headless activity/export schemas, removing the dedicated activity feed, redesigning workflow stages, changing pending task 082, or writing workflow state from Spacetop.

## Acceptance criteria

**AC-1:** Active and archived task rows show no runtime activity text or activity-only running/human-gate glyphs/colors for Idle, Running(Worker), Running(FirstOfficer), HumanGate or missing activity. Workflow stage, selection, worktree marker, title and durable gate indicator remain correct.

**AC-2:** Selected-task detail/preview header shows no runtime/session/activity-status/last-activity timestamp line across those states. Workflow status/score/source and durable gate facts remain available, and body content containing similar words is preserved.

**AC-3:** Ratatui TestBackend tests cover narrow/wide layouts, active/archive scopes and all runtime variants. Removing the suppression or accidentally removing workflow-stage/gate metadata must fail the assertions. Update user-facing docs and obsolete positive activity-display tests together.

**AC-4:** The change remains confined to these display surfaces and preserves keyboard behavior, read-only safety, core activity data and headless contracts. Run cargo fmt --check, cargo test and make lint before completion.

## Proof plan

Use existing ui/tests/task_list.rs and ui/tests/preview.rs plus shared runtime fixtures at the lowest rendering layer. Assert the rendered headers/rows rather than globally excluding words from user-authored markdown bodies. Keep durable gate assertions and ASCII/worktree/selection layout checks. No real watcher rerun is required unless watcher behavior changes.

## Stage Report: plan

- DONE: Plan removal of inferred runtime labels, markers and attribution line from list/preview only; name exact owned UI/tests/docs files and preserve workflow stages, durable gates and worktree/selection facts.
  Current render paths are `ui/list.rs:245-293` and both preview placement branches calling `push_session_attribution_line`; implementation is display-only.
- DONE: Specify TestBackend coverage for missing/Idle/Running worker/Running FO/HumanGate across active/archive and narrow/wide views, preserving body text and existing key/headless/read-only contracts.
  The matrix below compares rendered cells and styles against a missing-activity baseline, with positive workflow/gate/body assertions.
- DONE: Name the minimal implementation and falsifiable proof, docs changes, cargo fmt --check, cargo test and make lint; avoid backend/session-detection redesign or unrelated task changes.
  Exact owned files, executable verification commands and failure conditions are specified below; no Rust code changes or paid calls in this stage.

### Implementation sequence

1. Create an isolated task084 feature worktree from current main (observed base `329467e`); inspect its git status and policies before editing. Keep this entity/report in the shared state checkout. Do not edit main or task082.
2. In `crates/spacetop/src/ui/list.rs`, remove the activity lookup, running/human-gate marker/styles and status-label append. Preserve gutter selection, workflow phase/stage color, id, worktree marker, title and archived verdict. Remove only the activity column; adjust durable-gate span placement so it still appears before the title and test the resulting alignment.
3. In `crates/spacetop/src/ui/preview.rs`, remove both attribution-line calls, `push_session_attribution_line`, `session_attribution_line`, `format_latest_activity` and the now-unused time import. Preserve status/score, source, worktree/path, archived verdict, durable gate summaries/detail body, divider, wrapping and scrolling. Do not filter markdown body words.
4. Update `crates/spacetop/src/ui/tests/task_list.rs` and `crates/spacetop/src/ui/tests/preview.rs`; use `crates/spacetop/src/ui/tests.rs` only for a shared typed runtime-case fixture/helper. Replace obsolete positive activity-marker and metadata tests. Retain the scanner replay exercise by asserting typed index activity still transitions running-to-idle while rendered task rows stay unchanged; backend evidence must not be weakened to make rendering pass.
5. Update `README.md` to state that task rows and preview headers omit inferred runtime information while backend/headless contracts remain. Update the current contract paragraph in `docs/superpowers/specs/2026-07-27-spacetop-entity-activity-design.md` that presently claims preview Runtime/Session/Status/Updated. Keep evidence/detection rules intact. No dependency, schema, CLI, activity-feed, watcher or app lifecycle change is planned.

### Rendering proof

- Cross missing activity, Idle, Running(Worker), Running(FirstOfficer) and HumanGate with active/archive scopes and narrow/wide terminal sizes (e.g. 60x40 and 200x40). Include left/bottom preview placement, using direct render calls where a full-screen layout would hide one surface. Archive fixtures must actually carry supplied runtime data to exercise suppression, rather than passing because it is absent.
- List proof: compare the same row cells and styles with the missing-activity baseline; assert phase/id/title, selected gutter/background, stage color, worktree marker, archived verdict and a durable `[gate:...]` indicator at applicable widths. Runtime-only glyph/label/color reintroduction fails baseline equality. Use short fixture values so positive facts cannot disappear through truncation; retain existing ASCII/worktree/alignment tests.
- Preview proof: compare header cells/styles before the body divider with the missing-activity baseline. Assert status/score/source/worktree/path and durable gate facts positively, and runtime/session/activity-status/updated fields negatively within the header only. A body fixture containing `idle`, `running · worker`, `running · FO`, `human-gate`, `Runtime:`, `Session:` and `Updated:` must still render below the divider. Reintroducing attribution fails equality/absence checks; removing workflow metadata, gates or body words fails positive checks.
- Durable gate readiness is independent: retain/extend `durable_gates_list_is_independent_of_session_activity_at_narrow_and_wide_widths` and `durable_gates_preview_shows_current_historical_and_invalid_details_and_scrolls`. Exercise a genuine workflow gate separately from HumanGate runtime state; the latter must not replace or hide the former.
- Falsifiability check before production removal: new suppression assertions must fail against old rendering for Worker/FO/HumanGate rows and Idle/Running metadata. After removal they pass. Deliberately removing a workflow stage/gate assertion target would fail preservation tests; baseline equality alone is insufficient proof.

### Verification and boundaries

- Run `cargo test -p spacetop ui::tests::task_list`, then `cargo test -p spacetop ui::tests::preview`; record which claims fail before implementation and pass after it.
- Completion gates: `cargo fmt --check`, `cargo test`, `make lint`. Full tests retain keyboard/app behavior, headless schemas, terminal-free core and `no_write_git_calls` guardrails. No ignored real-watcher run is needed because watcher behavior is unchanged.
- Review the path-scoped diff: production changes belong only to list/preview; tests/helpers and two affected docs are the remaining expected files. Leave `session_activity`, index/domain models, headless exporters, `ui/activity.rs`, workflow markdown and unrelated task changes untouched by Spacetop implementation.

### Summary

The plan removes inferred runtime presentation through two narrow rendering edits and updates their tests and current docs. It preserves workflow and durable gate facts with positive assertions, and proves runtime independence across states, scopes and sizes without changing detection or headless data. This stage produces the implementation plan only; code and final test/lint evidence belong to the implementation stage.
