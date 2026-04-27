---
id: "035"
title: Auto-refresh overview on workflow file changes and post-merge updates
status: design
source: captain
started:
completed:
verdict:
score:
worktree:
issue:
pr:
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
