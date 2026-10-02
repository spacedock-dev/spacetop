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


## Stage Report: implement

- DONE: Remove runtime activity labels/markers/styles from list and runtime/session/status/updated attribution from both preview headers, preserving workflow metadata, durable gates, worktree/selection/archive facts and body text.
  AC-1/AC-2: code commit `2d47c1e` removes only list runtime spans/column and both preview attribution calls/helpers; tests positively retain stage color, selection, worktree, title, active gate readiness, archived verdict, historical gate briefing and user-authored runtime words.
- DONE: Add red-first TestBackend proof across missing/Idle/Running Worker/Running FO/HumanGate, active/archive and narrow/wide views; preserve backend/headless/activity-feed/key/read-only contracts and update affected docs.
  AC-3: `task_rows_ignore_runtime_across_scopes_and_widths` and `preview_ignores_runtime_preserving_metadata_and_body` compare cells/styles for all five variants, active/archive, 60/200 columns, and both preview placements; supplied archive runtime data is asserted present before rendering.
- DONE: Commit scoped deliverables on the isolated branch; pass cargo fmt --check, cargo test and make lint, then report all four ACs with falsifiable evidence and exact command results.
  AC-4: `2d47c1e` committed and pushed to `spacedock-ensign/hide-runtime-activity-in-task-views`; production changes are only list/preview, plus rendering fixtures/tests, one mouse fixture width and two docs; backend, headless, activity feed and workflow write paths unchanged.

### Reproducible evidence

- Red: `cargo test -p spacetop ui::tests::task_list` exited 101: 37 passed, 2 failed; runtime Worker changes row cells/styles and scanner replay shows the old marker. `cargo test -p spacetop ui::tests::preview` exited 101: 18 passed, 1 failed; Idle header contains runtime attribution.
- Green: same focused commands exited 0: list 39 passed and preview 19 passed. The replay test asserts index Running across reload/unchanged scans then Idle after terminal evidence, while task render buffers remain identical; changing scanner lifecycle or reintroducing labels fails it.
- Preservation mutation: `cargo test -p spacetop task_rows_ignore_runtime_across_scopes_and_widths` exited 101 (1 failed) after changing `[gate:{}]` to `[removed:{}]`; `cargo test -p spacetop preview_ignores_runtime_preserving_metadata_and_body` exited 101 (1 failed) after replacing the displayed stage with `removed-stage`. Both temporary mutations were restored.
- A first stage mutation revealed `review` also occurs in `Preview`; the final test asserts exact `status: ● review` and catches the mutation. Positive metadata/body assertions catch accidental removal rather than relying only on equal buffers.
- `cargo fmt --check`: exit 0. `cargo test`: exit 0, 685 passed, 0 failed, 4 ignored across unit/integration/doc suites. `make lint`: exit 0, runs `cargo clippy --all-targets --all-features -- -D warnings`. `git diff --check`: exit 0.
- Existing headless JSON/filter, keyboard/mouse, terminal-free core and `no_write_git_calls` tests passed. Ignored real-watcher rerun omitted because watcher behavior is unchanged.
- Removing the activity column expands the narrow ID reservation by two cells; Unicode/layout tests were updated. The mouse reflow test uses 96 instead of 100 columns to keep proving full-ID double click after the first click moves its original last cell outside the new ID rectangle; mouse production code unchanged.

### Plan assessment provenance

The FO supplied a prior paid plan assessment; this worker made no paid calls. Request SHA-256: `c5d32a29c272aa674bd24668cec0c2afd74e0f3f3514478a7b75fe2dcc10a019`; response SHA-256: `41956a948e590b4db5fb1b07704e6206ece397c7db0850dd70d922a6b74ed1e5`. Model `jev-1.13.0`; charge $0.000114282 of approved $0.01. Original response retained below; no raw source snapshot is included.

```json
{"model":"jev-1.13.0","answers":{"plan_1":{"type":"choice","choice":"supported","confidence":1.0,"probabilities":{"supported":1.0,"insufficient":0.0,"missing":0.0}},"plan_2":{"type":"choice","choice":"supported","confidence":1.0,"probabilities":{"supported":1.0,"insufficient":0.0,"missing":0.0}},"plan_3":{"type":"choice","choice":"supported","confidence":1.0,"probabilities":{"supported":1.0,"insufficient":0.0,"missing":0.0}}},"usage":{"input_tokens":2721,"output_tokens":123}}
```

### Summary

Task rows and both preview header layouts now omit inferred runtime presentation while keeping workflow and durable gate facts. Red/green and preservation mutation tests exercise real TestBackend cells/styles; full tests and lint pass. Code is committed and pushed on the isolated branch; archived current readiness remains absent under the existing contract while recorded gate details stay available.
