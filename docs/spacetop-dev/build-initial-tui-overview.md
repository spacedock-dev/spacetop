---
id: 003
title: Build Initial TUI Overview
status: plan
source: commission seed
started: 2026-04-24T14:50:32Z
completed:
verdict:
score: 0.8
worktree:
issue:
pr:
---

Build the first read-only `ratatui` overview that shows workflow stages, work item counts, and a selectable task list for a chosen Spacedock workflow directory.

## Acceptance criteria

**AC-1 -- The TUI renders a workflow overview from real markdown state.**
Verified by: running the binary against `docs/spacetop-dev` shows stage names and task counts derived from the workflow files.

**AC-2 -- Users can move selection through tasks without changing workflow files.**
Verified by: UI/event tests or a documented manual run confirm navigation changes selection only and `git diff docs/spacetop-dev` remains empty after viewing.

**AC-3 -- The selected task preview exposes useful state.**
Verified by: the overview displays the selected task title, status, score/source, and markdown body excerpt.
