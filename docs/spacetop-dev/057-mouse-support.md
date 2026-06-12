---
id: "057"
title: Mouse support for Spacetop panels
status: shape
source: captain request 2026-06-12
kind: feature
risk:
milestone:
proof:
started: 2026-06-12T06:36:10Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

Add mouse interaction to the Spacetop TUI so the captain can inspect Spacedock
workflow state without keyboard round-trips. Captain-confirmed behaviors:

1. **Click selects + opens preview** — a single left-click on an entity row in
   the overview list selects it and immediately opens/updates the preview
   sub-panel.
2. **Scroll wheel scrolls the preview** — when the preview panel is open, the
   mouse wheel scrolls its content (and the entity list when hovering it).
3. **Drag-divider resize** — click and drag the border between the list and
   the preview panel to resize the split, like a GUI splitter.
4. **Native text selection stays available via Shift+drag** — the app keeps
   mouse capture, relying on the standard terminal convention that Shift+
   left-drag bypasses capture so the terminal handles selection/copy natively
   (iTerm2, Terminal.app, kitty, WezTerm). No plain-drag passthrough is
   expected; this convention must be documented in the UI help surface.

Captain-selected scope: overview entity list, preview sub-panel, workflow
picker (click to choose a workflow), and stage graph (click a stage node).
Stage-graph interaction is the most speculative — shape may bound it to the
minimal useful click behavior or split it out as a follow-up if it bloats the
task.

## Scope

- Kind: feature
- Risk:
- Milestone:
- Touches: app-state / UI / docs
- Non-goals: (shape to fill — at minimum: no mutation of workflow files; no
  custom per-terminal mouse protocols beyond crossterm's standard capture)

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.
(Shape stage to formalize from the four captain-confirmed behaviors above.)

## Proof plan

- Lowest test layer:
- Required command:
- Manual check, if any:
- Docs/policy update needed:
