---
id: 045
title: Reload workflows live when README files are added, changed, or removed
status: plan
source: captain
started: 2026-05-26T15:08:52Z
completed:
verdict:
score:
worktree:
issue:
pr:
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
