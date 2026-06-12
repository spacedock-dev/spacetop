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
