---
id: "061"
title: Preview header gives source and worktree separate lines
status: plan
source: captain request 2026-06-12
kind: bugfix
risk: low
milestone: v1-maintenance
proof: Ratatui preview rendering regression plus make lint
started: 2026-06-12T12:53:54Z
completed:
verdict:
score: 0.66
worktree:
issue:
pr:
---

The preview page header currently renders fields such as `source: ...` and
`worktree: ...` in a compact header row. These values can be long, especially
when a task source names a detailed user request or a worktree path includes a
long branch/entity slug. The preview header should put `source:` and `worktree:`
on their own lines so long values do not crowd the rest of the header.

## Scope

- Kind: bugfix
- Risk: low
- Milestone: v1-maintenance
- Touches: UI
- Non-goals: changing entity parsing, changing task metadata semantics, changing
  workflow markdown, or adding any write behavior.

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 -- Preview source renders on its own header line.**
When the selected task has a long `source` value, the preview page header renders
`source: ...` on a dedicated line instead of sharing a compact metadata row with
other fields.
Verified by:

**AC-2 -- Preview worktree renders on its own header line.**
When the selected task has a `worktree` value, including the empty/default `-`
display, the preview page header renders `worktree: ...` on a dedicated line
instead of sharing a compact metadata row with `source` or other fields.
Verified by:

**AC-3 -- Long header metadata does not overlap or hide primary preview content.**
Long `source` or `worktree` text is clipped, wrapped, or otherwise constrained
according to the existing preview layout rules, and it does not overlap the
title, controls, body text, border, or neighboring UI.
Verified by:

**AC-4 -- Non-preview screens keep their current metadata layout.**
The change is scoped to the preview page header and does not unintentionally
alter task list rows, footer text, workflow tabs, picker, help popup, or
Workflow Definition rendering.
Verified by:

## Proof plan

- Lowest test layer: Ratatui `TestBackend` rendering regression for the preview
  page with long `source` and `worktree` metadata.
- Required command: `make lint`
- Manual check, if any: run `cargo run -p spacetop -- --workflow-dir docs/spacetop-dev`,
  open a task preview with long source/worktree values, and confirm the header
  remains readable.
- Docs/policy update needed: update help/footer text only if user-facing
  documented behavior changes.
