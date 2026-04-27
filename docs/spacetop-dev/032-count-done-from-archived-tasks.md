---
id: 032
title: Count done from archived tasks in workflow overview
status: plan
source: user request 2026-04-27
score: 0.4
worktree:
issue:
pr:
started: 2026-04-27T02:26:48Z
---

The workflow overview should not show a nonzero `#done` count for `docs/spacetop-dev` just because completed items exist in `_archive/`. When the workflow is shown in its active scope, `#done` should be `0`.

## Acceptance criteria

**AC-1 -- Active overview done count stays at zero.**
Verified by: the workflow overview for `docs/spacetop-dev` renders `done: 0` in the stage summary while active items remain unchanged.

**AC-2 -- Archived items do not affect active stage totals.**
Verified by: archived tasks are still available in archived scope, but they do not contribute to the active stage count shown in the workflow header.
