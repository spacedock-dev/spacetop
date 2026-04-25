---
id: "012"
title: Exclude .worktrees/ directories from workflow auto-discovery
status: design
source: feature request — first-officer session 2026-04-25
started:
completed:
verdict:
score: 0.7
worktree:
issue:
pr:
---

When Spacetop auto-discovers workflow directories (via `--discover` or directory scanning), it may walk into `.worktrees/` subdirectories and treat each worktree clone as an independent workflow. Each active Spacedock task checked out into a git worktree contains a full copy of the project including `docs/` — which means the same workflow directory appears N+1 times (once on main, once per active worktree). This causes duplicated workflow entries in the picker and graph view, and confuses the overview state.

## Context

Spacedock first-officer agents check out each `implement`-stage task into a dedicated worktree under `.worktrees/{worker-slug}/`. These worktrees are project-root replicas — they contain the same `docs/spacetop-dev/`, `docs/spacetop-ui/`, etc. as main. Spacetop's discovery currently has no exclusion for these paths.

Worktrees are ephemeral and not meant to be browsed as top-level workflow contexts.

## Proposed fix

In `src/discovery.rs` (or wherever `--discover` / directory scanning is implemented):
- Skip any path component that is `.worktrees` when recursing into workflow directories.
- Concretely: after resolving candidate dirs, filter out any path whose canonicalized form has `.worktrees` as an ancestor component.

Also consider: display a hint in the picker when a path is inside `.worktrees/`, or add a warning to the graph view header if the loaded workflow path is inside a worktree.

## Acceptance criteria

**AC-1 — `--discover` does not return paths under `.worktrees/`.**
Verified by: unit test or integration test in `tests/` that creates a fake `.worktrees/some-task/docs/workflow/` directory structure and asserts `--discover` output does not include it.

**AC-2 — A workflow opened at a project root that has active worktrees shows only one entry per distinct logical workflow (not N+1 for each worktree clone).**
Verified by: `cargo test` with a fixture that has both a real workflow dir and a `.worktrees/` sibling — picker item count equals real-workflow count.

**AC-3 — The filter does not break discovery when `.worktrees/` is absent.**
Verified by: existing discovery tests continue to pass with no modification.
