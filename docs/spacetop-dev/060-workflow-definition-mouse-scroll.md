---
id: "060"
title: Workflow Definition page supports mouse scroll wheel
status: plan
source: captain request 2026-06-12
kind: feature
risk: low
milestone: v1-maintenance
proof: app/input and UI regression plus make lint
started: 2026-06-12T12:55:32Z
completed:
verdict:
score: 0.62
worktree:
issue:
pr:
---

The Workflow Definition page should support mouse scroll wheel input, matching
the scroll behavior users expect from the task preview and other scrollable
panels. A user inspecting the workflow README/definition should be able to scroll
through longer definitions with the mouse wheel without switching back to
keyboard-only navigation.

## Scope

- Kind: feature
- Risk: low
- Milestone: v1-maintenance
- Touches: app-state / UI
- Non-goals: changing workflow definition parsing, changing task-list mouse
  behavior, adding workflow markdown write support

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 -- Workflow Definition view handles mouse wheel scrolling.**
When the Workflow Definition page is open and its content exceeds the visible
height, mouse wheel up/down adjusts the definition scroll offset.
Verified by:

**AC-2 -- Mouse wheel behavior stays scoped to the active page.**
Mouse wheel input in the Workflow Definition page does not accidentally scroll
the task list, preview pane, picker, or help overlay state.
Verified by:

**AC-3 -- Keyboard behavior remains unchanged.**
Existing keyboard controls for opening, closing, and scrolling the Workflow
Definition page continue to work as documented.
Verified by:

## Proof plan

- Lowest test layer: app/input tests for mouse wheel events on the Workflow
  Definition page, plus Ratatui rendering assertions if visible scroll state is
  exposed in the rendered page.
- Required command: `make lint`
- Manual check, if any: run `cargo run -p spacetop -- --workflow-dir docs/spacetop-dev`,
  open the Workflow Definition page, and scroll it with the mouse wheel.
- Docs/policy update needed: update help/footer text only if the user-facing
  mouse behavior is documented there.
