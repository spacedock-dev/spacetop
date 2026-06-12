---
id: "063"
title: Worktree copies of archived entities reappear as active tasks
status: plan
source: captain bug report 2026-06-12
kind: bugfix
risk: medium
milestone: v1-maintenance
proof: parser/app-state regression plus make lint
started: 2026-06-12T13:43:03Z
completed:
verdict:
score: 0.9
worktree:
issue:
pr:
---

Spacetop merges task files from workflow copies under `.worktrees/*` so an
active task can show a worktree marker and preview the task body from the
worker's branch while keeping root frontmatter as the status source. This breaks
when a worktree contains stale copies of other tasks that have already been
completed and archived on the root workflow.

Example:

- Root workflow has active task `0x0c` with `worktree:
  .worktrees/0x0c-xxxx`.
- Root workflow has tasks `0x0a` and `0x0b` completed and moved under
  `_archive/`.
- The `0x0c-xxxx` worktree still contains old `0x0a.md` and `0x0b.md` files in
  its mirrored workflow directory because that worktree branch was created
  before the archive moves.

Today, Spacetop can scan those worktree-only files and treat `0x0a` and `0x0b`
as active tasks again. That makes the active task list and status model
incorrect. Root workflow state and root archive placement should be authoritative
for membership and status. Worktree files should be an overlay for active task
content, not a way to resurrect archived tasks.

## Scope

- Kind: bugfix
- Risk: medium
- Milestone: v1-maintenance
- Touches: parser / index-query / app-state / UI
- Non-goals: changing Spacedock archive semantics, deleting stale files from
  worktrees, adding workflow markdown write support, or removing the existing
  worktree marker/preview feature for the active task being processed.

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 -- Root active and archive state decide active-list membership.**
If an entity is archived in the root workflow, a stale markdown copy of that
same entity under any scanned worktree does not make it appear in the active task
list.
Verified by:

**AC-2 -- Root frontmatter remains authoritative for active worktree-backed tasks.**
For an entity that is still active in the root workflow and has a worktree copy,
Spacetop keeps status, title, source, worktree path, issue, PR, and other
frontmatter from the root entity while using the worktree body only for preview
content and diff behavior.
Verified by:

**AC-3 -- Worktree overlay is scoped to the correct active entity.**
When a worktree belongs to active task `0x0c`, stale copies of unrelated archived
tasks such as `0x0a` and `0x0b` are ignored for active scope, while the `0x0c`
preview can still use the worktree body and show the worktree icon/path.
Verified by:

**AC-4 -- Archived scope stays anchored to root archive files.**
The archived view shows `0x0a` and `0x0b` from the root `_archive/` directory,
not from stale worktree copies, and archived counts remain consistent after a
refresh.
Verified by:

**AC-5 -- Legitimate worktree-only behavior is explicitly decided and tested.**
If Spacetop still supports worktree-only tasks that do not exist in root active
or root archive state, that behavior is documented in code/tests and does not
apply to entities whose slug or ID exists in root `_archive/`.
Verified by:

**AC-6 -- Spacetop remains read-only toward workflow markdown.**
The fix changes how Spacetop interprets root and worktree files, but it does not
delete stale worktree files or mutate workflow markdown.
Verified by:

## Proof plan

- Lowest test layer: `spacetop-core` parser/index regression with a fixture
  containing root active `0x0c`, root archived `0x0a`/`0x0b`, and a `0x0c`
  worktree that still contains all three files.
- App/UI coverage: focused app or Ratatui test proving the active list contains
  only `0x0c`, the worktree marker/preview behavior remains for `0x0c`, and the
  archived scope still contains `0x0a`/`0x0b`.
- Required command: `make lint`
- Manual check, if any: create or reuse a local workflow/worktree with the
  example shape, open `spacetop --workflow-dir <workflow>`, and confirm active
  and archived scopes match root state.
- Docs/policy update needed: update README or development policy only if the
  supported meaning of worktree-only tasks changes.
