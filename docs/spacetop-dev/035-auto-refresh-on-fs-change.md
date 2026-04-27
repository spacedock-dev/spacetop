---
id: "035"
title: Auto-refresh overview on workflow file changes and post-merge updates
status: review
source: captain
started: 2026-04-27T09:56:34Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-035-auto-refresh-on-fs-change
issue:
pr:
mod-block: merge:pr-merge
---

SpaceTop should detect filesystem changes to workflow entity files and refresh the overview without requiring user input. Two concrete scenarios:

1. An entity markdown file (`docs/{workflow}/*.md` or `docs/{workflow}/_archive/*.md`) is edited externally — by `status --set`, by another editor, or by a sibling tool. The TUI should pick up the new frontmatter / body within a short debounce window.

2. A worktree branch is merged back to `main` — the merge brings entity-file moves (e.g., into `_archive/`), frontmatter changes (e.g., `status: done`, `completed`, cleared `worktree`), and possibly new entity files. The TUI should detect the post-merge filesystem state and re-render.

Reference: `src/watcher.rs` (notify+poll fallback, debounce thread) and `src/app.rs::reload_from_snapshot` for the reload path. The watcher already exists; this task is about confirming both scenarios trigger a refresh — and fixing whichever path is gappy.

## Acceptance criteria

**AC-1 — External edits to an active entity file trigger a refresh.**
Verified by: an integration test that starts the watcher against a workflow fixture, mutates an entity file's frontmatter (`status: design` → `status: implement`), and asserts the test harness receives a refresh signal within the debounce window. Tests should not depend on a live terminal backend.

**AC-2 — Archive moves and additions trigger a refresh.**
Verified by: an integration test that simulates a merge by (a) renaming an entity file from the active workflow directory into `_archive/`, and (b) writing a new entity file into the active directory. Both events drive a refresh signal.

**AC-3 — Refresh path reloads via `App::reload_from_snapshot` without losing UI state.**
Verified by: an `app::tests` test that invokes `reload_from_snapshot` with a snapshot whose item set differs from the current one (additions, removals, status flips) and asserts (a) the new snapshot is reflected in `stage_counts()` and `archived_done_count`, and (b) the existing scope/selection clamp behavior remains correct (selection clamps without panicking when the visible list shrinks).

## Implementation plan

### Trigger path (already wired; preserve)

The reload trigger path for both external edits and post-merge filesystem changes is the same single seam:

1. `notify` (or `PollWatcher` fallback) sends raw `Event`s to `forward_events_to` in `src/watcher.rs:136`.
2. `debounce_loop` in `src/watcher.rs:204` filters via `event_is_relevant` → `is_relevant`, coalesces a burst over `DEFAULT_DEBOUNCE` (250 ms), then emits one `RefreshSignal` on `signal_tx`.
3. The main loop in `src/lib.rs:139-153` drains the receiver with `try_recv` each frame and calls `app.reload()`, which dispatches to `OverviewState::reload` (`src/app/overview.rs:191`) → on success, `OverviewState::reload_from_snapshot` (`src/app/overview.rs:142`).
4. `reload_from_snapshot` swaps the snapshot, invalidates the archived cache (`archived_items.clear()`, `archive_loaded = false`), re-runs `refresh_archived_done_count`, preserves selection by slug, and clamps if necessary — covering both the active edit path (AC-1) and the merge path (AC-2: archive moves + new files in active dir).

The watcher is started recursively against `app.workflow_dir()` in `start_watcher_for` (`src/lib.rs:210`), so events under both `docs/{workflow}/` and `docs/{workflow}/_archive/` reach the debounce loop. `is_relevant` already accepts `*.md` paths and slug-shaped directory names (covering directory rename events emitted by the merge), and rejects editor cruft.

### Implementation steps

This task is primarily a verification/test-coverage task — the watcher and reload seam exist. Steps:

1. **Audit `is_relevant` against merge-style events.** Confirm directory rename events (e.g., `_archive` materializing or an entity dir moving into `_archive/`) pass the filter. The current rule (markdown extension OR slug-shaped basename) covers `_archive` and folder-form `{slug}/index.md`. No change expected.
2. **Add AC-1 integration test.** Drives `WorkflowWatcher::start` against a workflow fixture, mutates `status:` frontmatter on an existing entity file, asserts a `RefreshSignal` arrives within the debounce window. New test in `tests/watcher_fs.rs` (real backend) — the existing real-backend test there is `#[ignore]`d; this one should be `#[ignore]`d too to keep CI deterministic, mirroring the sibling pattern. No `App` involvement; the test asserts the watcher signal only.
3. **Add AC-2 integration test.** Same harness shape: rename a file from the workflow root into a `_archive/` subdir, then write a brand-new entity file in the active dir; assert at least one `RefreshSignal` arrives. Co-locate in `tests/watcher_fs.rs`. Two FS mutations may collapse into a single signal under debounce (intended) — assert `recv_timeout` succeeds, not the count.
4. **Add AC-3 unit test.** In `src/app/tests.rs`, build an `App::from_snapshot` with N items, mutate selection (e.g., to last index), then call `app.reload_from_snapshot(new_snapshot)` where the new snapshot has different item set / different status distribution. Assert `stage_counts()` reflects the new items, `archived_done_count` is recomputed (Cell-not-applicable; field is `Option<usize>`), and `selected_index` clamps without panic when the new list is shorter. Use the existing `snapshot_with_items` helper / build inline.
5. **Run `make lint` and `cargo test` (plus `cargo test -- --ignored` locally for the new fs tests).** No code changes expected to `src/watcher.rs`, `src/app.rs`, `src/app/overview.rs`, or `src/lib.rs` — if a test surfaces a gap (e.g., directory-rename events not flagged relevant), patch the smallest possible scope: prefer extending `is_relevant` over expanding `reload_from_snapshot`.

### Test strategy

| AC | Test file | Test name | Asserts |
|----|-----------|-----------|---------|
| AC-1 | `tests/watcher_fs.rs` (`#[ignore]`) | `external_frontmatter_edit_triggers_refresh` | After `WorkflowWatcher::start` on a tempdir containing a workflow fixture, rewrite an entity `.md` file's frontmatter from `status: plan` to `status: implement`; `rx.recv_timeout(2s)` returns `Ok(_)`. |
| AC-2 | `tests/watcher_fs.rs` (`#[ignore]`) | `archive_rename_and_new_file_trigger_refresh` | After watcher start, (a) `fs::rename` an entity `.md` from workflow root into `_archive/` (creating dir if needed), (b) `fs::write` a new entity `.md` into workflow root; assert at least one `RefreshSignal` arrives within 2s (debounce may coalesce). |
| AC-3 | `src/app/tests.rs` (in-module) | `reload_from_snapshot_updates_counts_and_clamps_selection` | Start with `App::from_snapshot(dir, snap_a)` where `snap_a` has K items in mixed statuses; advance selection to last index; call `app.reload_from_snapshot(snap_b)` where `snap_b` has fewer items and different status distribution; assert (a) `app.stage_counts()` matches `snap_b`, (b) `app.selected_index() < snap_b.items.len()` (clamped, no panic), (c) `app.archive_error()` is unchanged or `None`. |

Notes:
- AC-1/AC-2 use the real `notify` backend and are gated `#[ignore]`, matching the existing `writes_to_markdown_trigger_refresh_signal` pattern in `tests/watcher_fs.rs`. Rationale: `notify` is platform/timing-sensitive; CI runs the deterministic in-memory `debounce_loop` tests in `src/watcher.rs::tests`, while the real-backend tests are run locally with `cargo test -- --ignored`.
- AC-3 stays in-module so it has access to crate-internal helpers and runs in default `cargo test`.

### Module/file ownership notes (implement worktree)

Implement-stage worktree owns:
- `src/watcher.rs` — only if step 1 audit reveals `is_relevant` misses a merge-style event class; otherwise no edits.
- `tests/watcher_fs.rs` — append AC-1 and AC-2 tests.
- `src/app/tests.rs` — append AC-3 test.
- `docs/spacetop-dev/035-auto-refresh-on-fs-change.md` — stage report only.

Stays out of scope:
- `src/app.rs`, `src/app/overview.rs`, `src/lib.rs` — the reload seam and event-loop wiring are correct as-is. Do not refactor.
- `src/parser.rs`, `src/domain/`, `src/discovery.rs`, `src/ui/` — unrelated to refresh path.
- Frontmatter writes — Spacetop is read-only (per `CLAUDE.md`); tests must not assert on or trigger any writes from Spacetop itself.

Boundaries to preserve:
- Watcher remains UI- and `App`-agnostic (per its module doc): it only emits `RefreshSignal`, never reaches into `App` state.
- `App::reload` / `OverviewState::reload_from_snapshot` remain the single reload seam used by both the watcher path and the workflow-switch path; do not introduce a parallel reload route for fs-driven refreshes.
- Parser/app/watcher boundary: the watcher does not parse; `OverviewState::reload` parses; `App` orchestrates. Keep it that way.

## Stage Report: plan

- DONE: Step-by-step implementation plan tied to src/watcher.rs (notify+poll fallback, debounce) and src/app.rs::reload_from_snapshot, naming the exact reload trigger path for both external edits and post-merge filesystem changes (incl. _archive moves and new files).
  Trigger path documented: `notify` → `debounce_loop` (`src/watcher.rs:204`) → `RefreshSignal` → main loop drain (`src/lib.rs:139-153`) → `App::reload` → `OverviewState::reload_from_snapshot` (`src/app/overview.rs:142`); recursive watch covers `_archive/` and folder-form entities.
- DONE: Test strategy that names a specific test for each of AC-1 (external edit refresh), AC-2 (archive rename + new file refresh), and AC-3 (reload_from_snapshot preserves scope/selection clamp), with target file locations (tests/ vs in-module #[cfg(test)]) and the assertions each test will make.
  Test matrix table specifies file, name, and assertions for each AC; AC-1/AC-2 in `tests/watcher_fs.rs` (`#[ignore]`), AC-3 in `src/app/tests.rs` in-module.
- DONE: Module/file ownership notes scoped to the implement worktree: which files implement vs test code may touch, what stays out of scope, and any parser/app/watcher boundary that must be preserved.
  Ownership section lists owned files (watcher.rs conditional, watcher_fs.rs, app/tests.rs), out-of-scope files (app.rs, overview.rs, lib.rs, parser.rs, ui/), and reaffirms watcher-is-UI-agnostic + single-reload-seam invariants.

### Summary

The auto-refresh seam already exists end-to-end: recursive `notify`/`PollWatcher` → `debounce_loop` → `RefreshSignal` → main-loop `app.reload()` → `reload_from_snapshot` (which already invalidates the archive cache, preserves selection by slug, and clamps). This task is primarily a verification gap: add two real-backend integration tests for AC-1/AC-2 in `tests/watcher_fs.rs` (gated `#[ignore]` per the existing convention) and one in-module test for AC-3 in `src/app/tests.rs`. No production code changes are anticipated unless the AC-2 audit reveals `is_relevant` misses a directory-rename event class, in which case the fix is scoped to extending the filter — not to the reload path.
