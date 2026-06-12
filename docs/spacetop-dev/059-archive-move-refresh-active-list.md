---
id: "059"
title: Archived task remains in active list until restart
status: plan
source: captain bug report 2026-06-12
kind: bugfix
risk: medium
milestone: v1-maintenance
proof: app/watcher refresh regression plus make lint
started: 2026-06-12T12:53:10Z
completed:
verdict:
score: 0.88
worktree:
issue:
pr:
---

When a workflow task file is moved into `_archive/`, a running Spacetop session
continues to show that task in the active task list until the app is restarted.
The expected behavior is that the filesystem refresh removes the task from the
active scope and makes it available only in the archived scope without requiring
a restart.

## Scope

- Kind: bugfix
- Risk: medium
- Milestone: v1-maintenance
- Touches: watcher / parser / app-state / UI
- Non-goals: adding a Spacetop archive/write action, changing Spacedock workflow
  markdown semantics, filing a GitHub issue

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 -- Archive moves remove tasks from the active list after refresh.**
When an active task file is moved from the workflow root into `_archive/`, the
running app updates its active task list after the normal refresh path without a
restart.
Verified by:

**AC-2 -- Archived scope shows the moved task after the same refresh.**
After the move, toggling to archived scope shows the moved task, and active vs
archived counts remain consistent with the workflow files on disk.
Verified by:

**AC-3 -- Spacetop remains read-only toward workflow markdown.**
The fix observes filesystem changes and reloads state, but does not add any
Spacetop path that mutates task markdown or archives tasks itself.
Verified by:

## Proof plan

- Lowest test layer: app-state or watcher-triggered reload test using a fixture
  workflow where an active task is moved into `_archive/`.
- Required command: `make lint`
- Manual check, if any: run `cargo run -p spacetop -- --workflow-dir docs/spacetop-dev`,
  archive a disposable task outside Spacetop, and confirm the active list updates.
- Docs/policy update needed: only if the reload behavior or keyboard/user-facing
  text changes.

## Implementation Plan

Goal: a watcher-driven reload must treat moving `task.md` to `_archive/task.md`
as a scope transition, not as an active task that lingers until process restart.

Root-cause search targets:

- Watcher: inspect `crates/spacetop-core/src/watcher.rs` and
  `crates/spacetop-core/tests/watcher_fs.rs` first. Confirm a rename into
  `_archive/` emits at least one `RefreshSignal` without needing a second active
  file write. If this is already true, keep watcher production code unchanged
  and rely on app/parser tests for the bugfix.
- Parser: inspect `crates/spacetop-core/src/parser/snapshot.rs`,
  `crates/spacetop-core/src/parser/archive.rs`, and
  `crates/spacetop-core/src/parser/worktree.rs`. Active loading already skips
  nested `_archive/` entries, so the risky path is worktree merging:
  `merge_worktree_items` currently treats a worktree copy with no active
  main-branch peer as worktree-only. After the main copy is archived, that can
  resurrect the task in the active list while the worktree still exists.
- App-state: inspect `crates/spacetop/src/app/overview.rs` and
  `crates/spacetop/src/app.rs`. `OverviewState::reload_from_index` invalidates
  archive cache and reloads archive immediately only when already in archived
  scope. The fix should preserve that ownership and make the active index come
  from corrected parser/index data.
- UI: inspect `crates/spacetop/src/ui/list.rs` only if the app-state regression
  passes but rendered rows are stale. The list should keep consuming
  `OverviewState::visible_items()` and must not infer `_archive/` semantics from
  paths.

Owned modules and likely change:

1. Add a parser-level archived-slug guard.
   Modify `crates/spacetop-core/src/parser/snapshot.rs` and
   `crates/spacetop-core/src/parser/worktree.rs` so worktree-only items are
   filtered out when the same slug exists under the main workflow's `_archive/`.
   Prefer collecting archived slugs from archive paths, not by parsing archived
   frontmatter, so malformed archived entries still suppress the stale active
   worktree row and archive parse errors remain owned by `archive.rs`.
2. Keep active/archive source separation.
   Do not load full archived entities into `WorkflowSnapshot`. The active
   snapshot should stay active-only; `WorkflowSources::load_archive` remains the
   archive entity loader used by app scope toggles and archive counts.
3. Preserve existing worktree behavior for non-archived items.
   Main-only items, worktree-only new items, divergent main/worktree pairs, and
   `.claude/worktrees/*` discovery should keep their current tests and semantics.
4. App reload should need little or no production change.
   If parser filtering fixes the stale active row, add only the app regression
   test. Change `OverviewState::reload_from_index` only if the archived cache is
   demonstrably stale after the parser fix.

Lowest practical regression tests:

1. Parser test in `crates/spacetop-core/src/parser/tests.rs`:
   create a temp repo with `docs/wf/task-001.md`, mirror the same file under
   `.worktrees/wt-1/docs/wf/task-001.md`, then move the main file to
   `docs/wf/_archive/task-001.md` while leaving the worktree file in place.
   Assert `load_workflow_dir(&wf, &root)` returns no active item with id `001`,
   while `load_archived_items(&wf, &allowed_statuses, None)` returns id `001`.
2. App-state test in `crates/spacetop/src/app/tests.rs`:
   load a real temp workflow through `App::load`, preload archive scope if useful
   to cover cache invalidation, perform the same archive move on disk, call
   `app.reload()` or `app.reload_with_rediscovery()`, assert the active snapshot
   no longer contains the moved id, toggle to archived scope with `a`, and assert
   the moved id is visible there after the same reload.
3. Watcher evidence:
   if the first investigation shows a rename alone is not signaled, add or adjust
   an ignored real-backend watcher test in
   `crates/spacetop-core/tests/watcher_fs.rs` for an archive rename without a
   companion active-file create. Otherwise cite the existing watcher coverage
   and keep the regression at parser/app layers.

Verification and read-only proof:

- Run the focused parser test first, expecting it to fail before the parser
  change and pass after it:
  `cargo test -p spacetop-core parser::tests::<new_test_name> -- --exact`
- Run the focused app regression:
  `cargo test -p spacetop app::tests::<new_test_name> -- --exact`
- Run broader non-ignored coverage after the focused tests:
  `cargo test`
- Run the required completion gate:
  `make lint`
- Run `cargo test -- --ignored` only if watcher production behavior or ignored
  watcher coverage changes.
- Prove read-only behavior by keeping all production changes inside parser and
  app reload/query code. Do not add any Spacetop command that writes workflow
  markdown, do not broaden `git_sync`, and do not touch editor/write paths.

## Stage Report: plan

- DONE: Plan names root-cause search targets and owned modules for archive-move refresh, separating watcher, parser, app-state, and UI responsibilities.
  See `Implementation Plan` sections `Root-cause search targets` and `Owned modules and likely change`.
- DONE: Plan identifies the lowest practical regression tests for active-list removal and archived-scope visibility after the same refresh.
  See `Lowest practical regression tests`, especially the parser worktree/archive test and app reload/toggle test.
- DONE: Plan explains how Spacetop remains read-only toward workflow markdown and how `make lint` evidence will be obtained or explicitly blocked.
  See `Verification and read-only proof`; no code was changed in this plan stage.

### Summary

Created a concrete implementation handoff for the archive-move refresh bug. The
plan isolates the likely stale active row to parser worktree merge semantics,
keeps archive visibility in app-state scope loading, and names focused tests plus
`make lint` as the completion gate for the later implementation stage.
