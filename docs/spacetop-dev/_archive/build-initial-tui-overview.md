---
id: 003
title: Build Initial TUI Overview
status: done
source: commission seed
started: 2026-04-24T14:50:32Z
completed: 2026-04-24T15:44:35Z
verdict: PASSED
score: 0.8
worktree: 
issue:
pr:
mod-block: 
archived: 2026-04-24T15:44:43Z
---

Build the first read-only `ratatui` overview that shows workflow stages, work item counts, and a selectable task list for a chosen Spacedock workflow directory.

## Acceptance criteria

**AC-1 -- The TUI renders a workflow overview from real markdown state.**
Verified by: running the binary against `docs/spacetop-dev` shows stage names and task counts derived from the workflow files.

**AC-2 -- Users can move selection through tasks without changing workflow files.**
Verified by: UI/event tests or a documented manual run confirm navigation changes selection only and `git diff docs/spacetop-dev` remains empty after viewing.

**AC-3 -- The selected task preview exposes useful state.**
Verified by: the overview displays the selected task title, status, score/source, and markdown body excerpt.

## Implementation plan

Build this as the first read-only terminal overview on top of the parser task's typed `WorkflowSnapshot`. Do not duplicate markdown/frontmatter parsing in UI code; the overview should consume domain records and keep selection/navigation state testable without a terminal backend.

1. Confirm the parser snapshot has landed or is available in the implementation worktree:
   - expected load API: `load_workflow_dir(&Path) -> Result<WorkflowSnapshot, _>` or equivalent
   - expected model data: stage names/metadata, work item `path`, `id`, `title`, `status`, `source`, `score`, and markdown `body`
   - if names differ, adapt the TUI boundary once in app state rather than throughout rendering code
2. Expand `src/app.rs` from a path holder into read-only UI state:
   - `App { workflow_dir, snapshot, selected_index, should_quit }`
   - `App::load(workflow_dir: PathBuf) -> Result<Self, _>` to call the parser
   - derived helpers for stage counts, selected item, next/previous selection, and clamped selection after empty/non-empty item lists
3. Keep navigation logic independent of terminal rendering:
   - define an app action method such as `handle_key(KeyEvent)` or `dispatch(Action)`
   - support `Up`/`k`, `Down`/`j`, `Home`, `End`, and `q`/`Esc`
   - ensure navigation only mutates in-memory selection and quit state
4. Replace `src/ui/mod.rs` placeholder rendering with a focused ratatui layout:
   - top or left summary block listing workflow directory and stage counts
   - task list block showing `id`, `status`, title, and a visible selected row
   - preview block showing selected item title, status, score/source, path, and a wrapped body excerpt
   - empty-state rendering for workflows with no work item files
5. Add the terminal runner at the CLI boundary:
   - keep `src/lib.rs::run(cli)` responsible for loading `App`, entering raw mode, drawing, polling events, and restoring the terminal
   - use `crossterm` + `ratatui::Terminal<CrosstermBackend<_>>`
   - do not add any workflow write paths or file mutation helpers
6. Handle parser and terminal errors clearly:
   - parser load failures should return an `anyhow` context naming the workflow directory
   - terminal setup/restore should avoid leaving the terminal in raw mode on normal errors
7. Update docs only if needed for an executable smoke command; otherwise keep implementation scoped to source and tests.
8. Run verification from the implementation worktree:
   - `cargo fmt --check`
   - `cargo test`
   - `cargo run -- --workflow-dir docs/spacetop-dev`
   - after exiting the TUI, `git diff -- docs/spacetop-dev` must show no changes

## Focused TUI and navigation verification strategy

Unit tests should cover app state and render output without requiring an interactive terminal session.

- App load/summary test: using `docs/spacetop-dev`, assert the app exposes the parser-derived workflow directory, expected stage names, and counts by current task status.
- Selection navigation tests: construct an app or fixture snapshot with multiple items and assert down/up/home/end behavior, bounds clamping, and empty-list behavior.
- Quit tests: assert `q` and `Esc` set `should_quit`, while movement keys do not.
- Render smoke tests: render the overview into a `ratatui::backend::TestBackend` and assert the buffer contains real stage names, task titles, selected status, score/source, and a body excerpt.
- No-mutation verification: before and after the manual smoke run, check `git diff -- docs/spacetop-dev`; expected result is empty because the overview is read-only.

## File and module ownership for implementation

The later TUI implementation worker should own only the overview/runtime surfaces and consume parser output.

- `src/app.rs`: app state, selection/navigation helpers, summary derivation, and related tests.
- `src/ui/mod.rs` or new files under `src/ui/`: ratatui layout/rendering and render-buffer tests.
- `src/lib.rs`: terminal event loop and parser/app wiring from `Cli`.
- `src/cli.rs`: only if an explicit non-interactive or display flag is required; no CLI change is currently necessary.
- `README.md`: only if documenting the smoke command becomes part of implementation evidence.

Parser/domain ownership remains with the parser task/worktree: `src/domain/mod.rs`, any `src/parser*` module, parser error types, and parser fixtures/tests. Workflow state files under `docs/spacetop-dev` must not be changed by the TUI implementation except for the assigned entity's own Spacedock stage report when dispatched.

## Stage Report: plan

- DONE: DONE/SKIPPED/FAILED accounting must show a concrete implementation plan for a read-only ratatui overview using the parser snapshot.
  Added an implementation plan that consumes `WorkflowSnapshot`, defines app state/navigation, replaces placeholder rendering, wires a terminal runner, and includes concrete verification commands.
- DONE: DONE/SKIPPED/FAILED accounting must show a focused TUI/navigation verification strategy, including no workflow-file mutation.
  Added unit, render-buffer, manual smoke, and `git diff -- docs/spacetop-dev` checks to prove navigation changes memory only.
- DONE: DONE/SKIPPED/FAILED accounting must identify file/module ownership for the later implementation stage.
  Added ownership notes for `src/app.rs`, `src/ui/*`, `src/lib.rs`, optional docs/CLI surfaces, and explicitly left parser/domain files to the parser task.

### Summary

Planned the initial overview as a read-only ratatui layer over parser-owned workflow data. The plan keeps selection and summary logic testable outside a terminal session, limits implementation ownership to app/UI/runtime files, and requires evidence that browsing does not mutate Spacedock markdown state.

## Stage Report: implement

- DONE: DONE/SKIPPED/FAILED accounting must show the TUI renders real workflow state from parser data, including stage counts and task preview content.
  `cargo test` passed 16/16; render test asserts `implement: 1`, `done: 0`, title, status, score/source, and body excerpt from `docs/spacetop-dev`.
- DONE: DONE/SKIPPED/FAILED accounting must show navigation/quit behavior is tested without mutating workflow files.
  App tests cover Down/Up/Home/End clamping plus `q`/Esc quit state, and smoke run left `git diff -- docs/spacetop-dev` empty before this report was appended.
- DONE: DONE/SKIPPED/FAILED accounting must show fresh `cargo fmt --check`, `cargo test`, and smoke/no-mutation evidence (`git diff -- docs/spacetop-dev`).
  Ran `cargo fmt --check`, `cargo test`, `expect -c 'spawn cargo run -- --workflow-dir docs/spacetop-dev; ... send "q"'`, and `git diff -- docs/spacetop-dev` with no workflow-state changes before report append.

### Summary

Implemented the initial read-only ratatui overview over parser-owned `WorkflowSnapshot` data. The app now loads workflow state, derives stage counts, tracks in-memory selection and quit state, renders summary/list/preview panes, and runs through a crossterm terminal loop that restores terminal state on exit.

## Stage Report: review

- FAILED: DONE/SKIPPED/FAILED accounting must show whether AC-1 (TUI renders real markdown workflow state) is satisfied with fresh evidence.
  REJECTED: fresh `cargo test` failed 14/16 because tests still expect `implement: 1`, while the real workflow file is now `status: review` after commit `490bde8`.
- DONE: DONE/SKIPPED/FAILED accounting must show whether AC-2 (navigation does not mutate workflow files) is satisfied with fresh evidence.
  `expect -c 'spawn cargo run -- --workflow-dir docs/spacetop-dev; ... send "q"'` exited 0, and `git diff -- docs/spacetop-dev` was empty before this report append.
- FAILED: DONE/SKIPPED/FAILED accounting must show whether AC-3 (selected task preview exposes useful state) is satisfied with fresh evidence, plus `cargo fmt --check`, `cargo test`, and smoke/no-mutation results.
  `cargo fmt --check` passed, smoke/no-mutation passed, but `cargo test` failed in `app::tests::loads_real_workflow_state_and_derives_stage_counts` and `ui::tests::renders_real_workflow_summary_task_list_and_preview`.

### Summary

Verdict: REJECTED. The implementation appears scoped to the expected app/UI/runtime files and the PTY smoke run did not mutate workflow files, but the automated evidence is not currently reproducible because the tests are coupled to mutable workflow status in `docs/spacetop-dev`.

## Stage Report: implement (cycle 2)

- DONE: DONE/SKIPPED/FAILED accounting must show the app/UI tests no longer depend on a transient live workflow status like `implement: 1`.
  Updated app/UI real-workflow tests to derive status/count expectations from the loaded snapshot instead of hard-coding `implement: 1`.
- DONE: DONE/SKIPPED/FAILED accounting must show the original TUI acceptance coverage remains: real workflow summary, selected task preview, navigation, and read-only behavior.
  `cargo test` passed 16/16, including real workflow summary/preview render tests plus navigation and quit-state tests.
- DONE: DONE/SKIPPED/FAILED accounting must show fresh `cargo fmt --check`, `cargo test`, and smoke/no-mutation evidence after the fix.
  Ran `cargo fmt --check`, `cargo test`, `expect -c 'spawn cargo run -- --workflow-dir docs/spacetop-dev; ...'`, and `git diff -- docs/spacetop-dev` was empty before this report append.

### Summary

Fixed the review rejection by removing exact-count/status assertions tied to mutable Spacedock workflow state. The tests still load and render the real workflow, but expected stage lines and preview status now come from the same snapshot under test, while fixture-based navigation coverage remains unchanged.

## Stage Report: review (cycle 2)

- DONE: DONE/SKIPPED/FAILED accounting must show whether AC-1 (TUI renders real markdown workflow state) is satisfied with fresh evidence.
  PASSED: `cargo test` passed 16/16, including real `docs/spacetop-dev` load/render tests whose stage counts are derived from the loaded snapshot.
- DONE: DONE/SKIPPED/FAILED accounting must show whether AC-2 (navigation does not mutate workflow files) is satisfied with fresh evidence, including no live-status test coupling.
  PASSED: navigation tests mutate only selection/quit state, `rg` found no stale `implement: 1` or `status: implement` test coupling, smoke exited via `j`, `k`, `q`, and `git diff -- docs/spacetop-dev` was empty before this report append.
- DONE: DONE/SKIPPED/FAILED accounting must show whether AC-3 (selected task preview exposes useful state) is satisfied with fresh evidence, plus `cargo fmt --check`, `cargo test`, and smoke/no-mutation results.
  PASSED: render test asserts selected title, status, score, source, and body excerpt; `cargo fmt --check`, `cargo test`, and the `expect` smoke run all exited 0.

### Summary

Verdict: PASSED. The feedback fix addresses the prior rejection by deriving live workflow status/count expectations from parser output, while preserving coverage for summary rendering, selected preview content, navigation behavior, and read-only smoke/no-mutation evidence.
