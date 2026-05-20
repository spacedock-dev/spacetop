---
id: 041
title: View the active workflow's definition from the overview page
status: design
source: captain request — inspect workflow structure (stages, gates, feedback edges) without leaving the TUI
started: 2026-05-20T08:13:05Z
completed:
verdict:
score:
worktree:
issue:
pr:
mod-block:
---

Spacetop already renders the *state* of a workflow (entities, stages, feedback arrow) but the captain has no in-TUI way to see the workflow's *definition* — the README frontmatter that declares stages, their `initial`/`terminal`/`gate`/`fresh`/`worktree`/`feedback-to`/`concurrency` properties, the entity labels, and the prose blocks under each `### {stage}` heading. Today the only path is to quit and `cat docs/{workflow}/README.md`.

This task adds an in-TUI affordance, accessible from the task (overview) page, that opens a detail view of the currently-selected workflow's definition. The view should make the structural decisions legible at a glance (which stage is initial, which is terminal, which is gated, where the reject edges go, which stages own a worktree) and let the captain read the per-stage prose without leaving the TUI.

## Open questions (design stage to resolve)

- Trigger: dedicated key (e.g. `W` for workflow detail) versus reusing the existing picker overlay's selection action.
- Render surface: full-page replacement (like the entity preview) versus a modal overlay (like the help popup).
- Content layout: how to surface stage properties compactly without re-implementing the existing stage-graph rendering.
- Scope: definition-only (frontmatter + prose) versus also exposing computed/derived facts like discovered entity counts per stage.

## Acceptance criteria

**AC-1 — Affordance is discoverable from the overview page.**
Verified by: keymap entry visible in the in-TUI help affordance; integration test or unit test in `src/app/keys.rs` that maps the chosen key to the new workflow-detail mode.

**AC-2 — The view renders the structural fields the captain cares about.**
Verified by: a unit test against parsed `WorkflowDefinition` / `StageDefinition` confirming the view exposes, per stage, the `initial`, `terminal`, `gate`, `fresh`, `worktree`, `feedback-to`, and `concurrency` flags as set in the README (no silent drops).

**AC-3 — Per-stage prose from the README is reachable.**
Verified by: a parser/UI test that the `### {stage}` body sections (Inputs/Outputs/Good/Bad bullets) round-trip from `README.md` into the detail view without being truncated or paraphrased.

**AC-4 — Read-only invariant preserved.**
Verified by: no write paths added to `discovery.rs` or `parser.rs`; the new module touches only render code and app-state transitions. Spot-checked by `make lint` plus the existing read-only guardrail in `CLAUDE.md`.
