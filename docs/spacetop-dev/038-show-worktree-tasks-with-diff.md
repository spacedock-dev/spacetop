---
id: 038
title: Show worktree-only tasks and body diffs in the task list
status: plan
source: captain request 2026-05-12
score:
worktree:
issue:
pr:
started: 2026-05-12T07:30:23Z
---

The SpaceTop overview currently lists tasks discovered only from the workflow's root directory. When a task is being worked on in an isolated worktree (e.g., `.worktrees/<slug>/docs/spacetop-dev/<slug>.md` or `.claude/worktrees/<name>/docs/spacetop-dev/<slug>.md`), edits to that task's body and frontmatter live on the worktree branch and are invisible to a captain browsing the main checkout. Spacetop should also scan worktree copies of the workflow so worktree-only tasks appear in the list, and so the preview surfaces where the in-flight body diverges from `main`.

Behavior:

- The task list includes tasks that exist only in a worktree copy of the workflow directory. These rows are visually marked as the "worktree version" so the captain can tell them apart from main-tracked tasks.
- When the same task (matched by slug or id) exists in both the root workflow directory and one or more worktree copies, the row uses the **root** copy's frontmatter (status, title, score, etc.) — the main branch remains the source of truth for state display.
- When the root and worktree copies of a task have different bodies, the preview pane shows a diff between the two bodies so the captain can see what the in-flight work has changed.
- Spacetop remains read-only: scanning worktrees never mutates workflow files in either location.

## Acceptance criteria

**AC-1 -- Worktree-only tasks appear in the task list with a distinguishing marker.**
Verified by: an integration test that builds a fixture with a task file present in a worktree copy of the workflow directory but not in the root workflow dir, runs discovery + overview state construction, and asserts the rendered task list contains the task with a visible marker (badge, suffix, or column indicator) that distinguishes worktree-sourced rows from root-sourced rows. The exact label/style is chosen during the design stage.

**AC-2 -- When a task exists in both root and worktree, status uses the root copy.**
Verified by: a unit test in the discovery/parser layer that supplies the same slug from both a root path and a worktree path with different `status:` values in their frontmatter, and asserts the merged task surfaced to the app uses the root copy's `status` (and other frontmatter fields) for list display, while still recording that a worktree copy exists.

**AC-3 -- Preview shows a body diff when root and worktree copies differ.**
Verified by: a test in `src/app.rs` / `src/ui/` (or wherever preview state lives) that constructs a task with a root body and a divergent worktree body, opens the preview, and asserts the rendered preview content contains a diff representation (added/removed lines or equivalent unified-diff structure) of the two bodies rather than only the root body. When bodies are identical, the preview falls back to the existing single-body rendering.

**AC-4 -- Worktree scanning is read-only and does not mutate workflow files.**
Verified by: the existing read-only invariant tests (or a new assertion) confirm that discovering and merging worktree task copies performs no writes under either the root workflow directory or the scanned worktree paths.

**AC-5 -- Worktree discovery handles the absence of worktree directories and missing per-worktree workflow paths gracefully.**
Verified by: a discovery-layer test that exercises (a) a repository with no worktrees registered, (b) a worktree that does not contain the workflow directory at all, and (c) a worktree containing an unrelated subset of task files; asserts no errors are raised and the task list matches the root view in each case (no spurious rows, no panics, no IO errors surfaced to the user).
