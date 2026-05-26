---
id: 045
title: Reload workflows live when README files are added, changed, or removed
status: implement
source: captain
started: 2026-05-26T15:08:52Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-045-reload-workflow-on-readme-change
issue:
pr: #43
---

Spacetop already runs a `notify`-based file watcher (`src/watcher.rs`) that picks up entity file changes inside a workflow directory. The captain reports that changes to the workflow's own `README.md` are not reflected live: editing `stages.states` / `stages.transitions`, creating a brand new workflow directory under the discovery root, or removing one, requires restarting `spacetop` before the picker or the DAG view shows the new shape.

The expected behavior is that when a watched workflow's `README.md` changes, is created, or is removed, the in-memory `WorkflowDefinition` (stages, transitions, colors, terminal markers) is re-parsed and the on-screen overview, stage graph, and picker re-render without a manual restart.

Likely touch points:

- `src/watcher.rs` — the watch set today is scoped to entity files in `{workflow_dir}/*.md`. The README is at the same depth but may not be in the watch set, or its events may be filtered. Confirm the actual event filter and broaden it.
- `src/discovery.rs` — picker discovery currently runs once at startup. Live add/remove of a workflow under the discovery root requires re-discovery on watcher events.
- `src/parser.rs` / `src/domain/` — re-parse path must be safe to call repeatedly; failed parses must not poison the prior good `WorkflowDefinition`.
- `src/app.rs` — `App` state needs an "apply re-parse" path that swaps the definition in place and invalidates any cached layout in the overview / graph.

## Acceptance criteria

Each AC names a property of the finished entity (not a stage action) and how it is verified.

**AC-1 — README edits trigger a live re-parse.**
Modifying a currently-open workflow's `README.md` causes the in-memory `WorkflowDefinition` to re-parse within one watcher debounce window, and the stage graph + overview reflect the new state without a key press or restart.
Verified by: an integration test under `tests/` that writes an initial README, opens it via `decide_app` / the watcher path, edits the README on disk, and asserts the post-event `App` state contains the updated stages / transitions.

**AC-2 — New workflow directories appear in the picker without restart.**
Creating a new `{some-workflow}/README.md` under the discovery root makes the workflow visible in the picker on the next watcher tick (and selectable) without restart.
Verified by: an integration test that boots discovery against a temp root with one workflow, creates a second workflow directory + README mid-session, and asserts the picker model includes the new workflow.

**AC-3 — Removing a workflow does not leak stale handles.**
Deleting a workflow's `README.md` (or its containing directory) removes the workflow from the picker and the overview without panicking. If the removed workflow was the active selection, the UI falls back to a clear empty / "workflow gone" state.
Verified by: an integration test that opens a workflow, removes its README, and asserts the app reaches a non-panicking post-removal state.

**AC-4 — Parse failures during live reload preserve the prior good state.**
A malformed README (bad YAML, missing `stages:` block) during a live reload does NOT replace the prior `WorkflowDefinition`. The prior state is retained and a visible warning is surfaced in the UI status area.
Verified by: a unit or integration test that loads a valid README, writes a broken README to disk, and asserts the in-memory definition is unchanged and a warning is recorded.

**AC-5 — `make lint` and `cargo test` remain clean.**
Verified by: running `make lint` and `cargo test` locally.

## Implementation Plan

### Design summary

The watcher already produces `RefreshSignal` events for any `*.md` file under its watched root, including `README.md` (the `is_relevant` filter accepts the `.md` extension regardless of basename). Two real gaps remain:

1. **Watch root scope.** `start_watcher_for` (in `src/lib.rs`) watches `app.workflow_dir()`, i.e. the active workflow's own directory. Edits to that workflow's `README.md` fire `RefreshSignal`, but the current `App::reload()` path only calls `OverviewState::reload` → `load_workflow_dir`, which does re-parse the README and build a fresh `WorkflowDefinition`. So AC-1 (README *edits* re-parse) is already wired in principle but is not test-locked end-to-end. We will add the integration test and confirm. The watch root does *not* currently include the discovery scan root, so creating a *new* workflow directory under the picker root is invisible (AC-2 gap).
2. **Re-discovery on add/remove.** Discovery runs once at startup and again only when the user opens the picker overlay with `P`. Adding or removing a workflow's `README.md` needs to drive re-discovery automatically when the watcher fires. The current `App::reload()` does not touch `OverviewSession::discovery`.

To keep parser and app-state changes testable without a TUI, we split the work cleanly between three layers: a tiny "scope" extension in `watcher`/`lib`, a re-discovery hook on `App`/`OverviewSession`, and parser preservation logic in `OverviewState::reload` (which already mostly behaves correctly — see AC-4 unit).

### Unit 1 — Broaden the watcher to also observe the discovery scan root

**Files:** `src/lib.rs` (`start_watcher_for`).

In multi-workflow sessions (`OverviewSession::scan_root().is_some()`), watch the scan root instead of just the active workflow dir. The existing `notify::RecursiveMode::Recursive` already covers the entire subtree, so edits to any workflow's `README.md`, and creates/removes of new workflow directories under the root, all flow through the same `RefreshSignal` channel.

For single-workflow / `-w pinned` sessions (`scan_root().is_none()`), keep the current behavior of watching `app.workflow_dir()` — there is nothing to discover.

`is_relevant` already accepts `.md` (covering README.md) and slug-shaped directory basenames (covering new workflow dirs); no watcher filter changes are required.

### Unit 2 — Introduce `App::reload_with_rediscovery(&mut self) -> Result<(), ParseError>`

**Files:** `src/app.rs`, `src/app/session.rs`.

Add a single method on `App` that the event loop calls instead of `App::reload()` when a `RefreshSignal` arrives. It does two things, in order, with no UI/terminal calls:

1. If the session has a `scan_root`, re-run `discovery::discover_workflows(scan_root)`. Apply the resulting list via `OverviewSession::replace_discovery` (already implemented — preserves loaded slots by canonical path match, falls back to index 0 if the prior active was removed). On discovery error, record it via `set_refresh_error` and skip re-discovery (do not poison existing state).
2. Reload the active slot from disk. If the active workflow's README/directory has been removed (i.e. the active slot's `workflow_dir()` no longer exists), substitute an `OverviewState::empty(dir)` with `last_refresh_error` set to a fixed "workflow removed" string so AC-3 can observe a non-panicking empty state. Otherwise call `OverviewState::reload` — its existing implementation already preserves the prior good snapshot on parse failure and records `last_refresh_error` (AC-4).

This method is pure state manipulation: no terminal I/O, no watcher restart. It is testable from `tests/` by constructing an `App::from_session` with a temp scan root, mutating files on disk, calling `reload_with_rediscovery`, and asserting on `as_session()` / `as_overview()`.

### Unit 3 — Wire the event loop to call the new method

**Files:** `src/lib.rs` (`run_terminal`).

In the `RefreshSignal` drain block, replace `let _ = app.reload();` with `let _ = app.reload_with_rediscovery();`. The block already runs synchronously on the main loop thread, so no new threading concerns.

When the active workflow's directory disappears, the watcher may emit further events for that path; the next drain reruns rediscovery and the active slot remains the synthetic empty state until the user picks another workflow (AC-3).

### Unit 4 — Tests (`tests/readme_reload.rs`)

Integration tests live in a new `tests/readme_reload.rs` file. They drive `App` directly, do not start a `Terminal`, and use the real `notify` backend behind `WorkflowWatcher::start`. To avoid CI flake on the live backend, the tests assert behavior on the *post-call* state of `App::reload_with_rediscovery` after a deterministic file mutation and a brief sync delay — i.e. they don't rely on the debounce channel firing; they prove the reload path is correct.

For each AC we add one named test (paths and assertions below cite the entity file's AC text verbatim):

- **AC-1 — `readme_edit_reparses_definition_live`** (`tests/readme_reload.rs:readme_edit_reparses_definition_live`)
  - Fixture: tempdir with `docs/wf-alpha/README.md` containing two stages (`design`, `done`) and a `commissioned-by: spacedock@0.10.1` marker.
  - Build a multi-workflow session via `OverviewSession::from_discovery` (one workflow, scan_root = tempdir root); call `App::from_session`; assert initial stage count is 2.
  - Overwrite `README.md` with a three-stage definition (`design`, `plan`, `done`).
  - Call `App::reload_with_rediscovery()`; assert `app.snapshot().definition.stages.iter().map(|s| s.name.as_str()).collect::<Vec<_>>() == vec!["design", "plan", "done"]`.

- **AC-2 — `new_workflow_directory_appears_in_session`** (`tests/readme_reload.rs:new_workflow_directory_appears_in_session`)
  - Fixture: tempdir scan root with one workflow `docs/alpha/README.md`. Build the session as above.
  - Create a second workflow at `docs/beta/README.md` (with the spacedock commission marker).
  - Call `App::reload_with_rediscovery()`; assert `app.as_session().unwrap().discovery().len() == 2` and that both `alpha` and `beta` roots are present (use canonicalized path equality).

- **AC-3 — `removing_active_workflow_yields_empty_state_without_panic`** (`tests/readme_reload.rs:removing_active_workflow_yields_empty_state_without_panic`)
  - Fixture: two workflows discovered (`alpha` active, `beta` second).
  - Remove `alpha/README.md` (or the whole `alpha/` dir) on disk.
  - Call `App::reload_with_rediscovery()`; assert no panic; assert `app.as_session().unwrap().discovery()` contains only `beta`; assert `app.workflow_dir()` is now `beta` (since `replace_discovery` falls back to the surviving entry and the active state is reloaded from there). If the only workflow is removed instead, assert that `as_overview().unwrap().snapshot().definition.stages` is empty and `last_refresh_error()` is `Some(_)`.

- **AC-4 — `malformed_readme_preserves_prior_definition`** (`tests/readme_reload.rs:malformed_readme_preserves_prior_definition`)
  - Fixture: one workflow with a valid three-stage README; build the session and snapshot the initial `stages` vec.
  - Overwrite the README with broken YAML (`stages: [\n` — unterminated flow).
  - Call `App::reload_with_rediscovery()`; assert `app.snapshot().definition.stages` equals the snapshotted prior stages exactly; assert `app.last_refresh_error()` returns `Some(msg)` where `msg` mentions "YAML" or the parser path. This is the AC-4 isolation property — the prior good `WorkflowDefinition` survives the failed re-parse.

- **AC-5** is covered by `make lint` and `cargo test` runs at implement-stage close; no new test file beyond the lint gate.

A unit test in `src/app.rs#tests` also exists to lock the "remove active" branch in `reload_with_rediscovery` against a fixture that does not touch the live `notify` backend, mirroring the discipline used by other `App` unit tests.

### How parse failure is isolated (AC-4 specifically)

`OverviewState::reload` (already in tree at `src/app/overview.rs:239-251`) calls `load_workflow_dir`, and only on `Ok(snapshot)` does it call `reload_from_snapshot`. On `Err`, it sets `self.last_refresh_error` and returns the error without touching `self.snapshot` — so `self.snapshot.definition` (the prior good `WorkflowDefinition`) is byte-equal to its pre-call value. `App::reload_with_rediscovery` propagates this contract: it discards the `Err` from `OverviewState::reload`, leaving the prior good state intact and the warning visible via `last_refresh_error()`.

The re-discovery step does not call back into the parser at all (it only walks `commissioned-by:` markers), so a malformed README in a *non-active* workflow likewise cannot corrupt the active slot. The active slot is reloaded last and through the same isolation path.

### Sequencing

1. Unit 1 — broaden watch root in `start_watcher_for`. No test change required (existing tests still cover the single-workflow path); manual confirmation via `cargo run` against `docs/spacetop-dev`.
2. Unit 2 — `App::reload_with_rediscovery`; add a focused unit test in `src/app.rs#tests` for the "remove active" branch using `OverviewState::empty` and a tempdir.
3. Unit 4a — write `tests/readme_reload.rs` with all four integration tests; iterate until green.
4. Unit 3 — swap the call site in `run_terminal`. Single-line change.
5. AC-5 — `make lint` and `cargo test`. Fix any clippy diagnostics inline.

### Module/file ownership

- `src/watcher.rs` — unchanged.
- `src/lib.rs` — modify `start_watcher_for` (Unit 1) and the `RefreshSignal` drain (Unit 3).
- `src/app.rs` — add `App::reload_with_rediscovery` (Unit 2). No frontmatter or scaffolding touched.
- `src/app/session.rs` — unchanged (uses existing `replace_discovery` and `install_active_state` APIs).
- `src/app/overview.rs` — unchanged (existing `reload` already preserves prior state on parse failure).
- `src/parser/*` — unchanged.
- `tests/readme_reload.rs` — new file; one test per AC-1..AC-4.

### Risks and mitigations

- **Test flake from real `notify` backend.** Mitigated by calling `App::reload_with_rediscovery` directly in the integration tests rather than waiting on `RefreshSignal`. The watcher itself is covered by existing `tests/watcher_fs.rs#[ignore]` tests.
- **Watch root regression on `-w pinned`.** The single-workflow branch keeps watching `workflow_dir()` to avoid pulling unrelated trees into the watch set. AC-2 only applies when a discovery scan root exists, which matches the captain's report.
- **`replace_discovery` selection drift.** When the active workflow is removed mid-session, `replace_discovery` falls back to index 0. If the user's prior `selected_index` inside that prior active state was non-zero, the new active slot is a fresh load (or empty state), so selection drift is contained to "new workflow, fresh selection" rather than an out-of-bounds panic.

## Stage Report: plan

- DONE: Plan separates watcher/discovery/parser changes from App-state swap so parser and state are testable without a TUI session.
  Units 1 (watcher root in `start_watcher_for`), 2 (`App::reload_with_rediscovery` in `src/app.rs`), and 3 (event-loop call site) are explicitly partitioned; parser code stays unchanged because `OverviewState::reload` already isolates parse failures.
- DONE: Test strategy names a concrete integration test for each of AC-1 through AC-4, citing fixture path and assertion.
  See `tests/readme_reload.rs:readme_edit_reparses_definition_live`, `:new_workflow_directory_appears_in_session`, `:removing_active_workflow_yields_empty_state_without_panic`, `:malformed_readme_preserves_prior_definition`.
- DONE: Plan specifies how a failed re-parse is isolated so the prior good WorkflowDefinition is preserved (AC-4).
  Section "How parse failure is isolated" pins the contract on `OverviewState::reload` (src/app/overview.rs:239-251) which mutates `self.snapshot` only on `Ok`, plus the unrelated-workflow argument that re-discovery does not invoke the entity parser.

### Summary

Plan adopts a three-unit split: broaden the watcher to the discovery scan root (multi-workflow only), add a UI-agnostic `App::reload_with_rediscovery` that runs re-discovery then per-active reload, and swap the existing `app.reload()` call in `run_terminal`. Parse-failure isolation rides on the existing `OverviewState::reload` contract — no parser changes needed. Four integration tests in a new `tests/readme_reload.rs` lock AC-1..AC-4 by driving `App` directly without a terminal backend.
