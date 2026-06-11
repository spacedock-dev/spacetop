---
id: "053"
title: "v2 P3: capability views"
status: verify
source: "captain - reviewed SpaceTop v2 roadmap plan"
kind: feature
risk: medium
milestone: v2-p3
proof: "cargo test --workspace; make lint; cargo test -p spacetop-core --test no_terminal_deps"
started: 2026-06-11
completed:
verdict:
score: 0.9
worktree: .worktrees/spacedock-ensign-053-v2-p3-capability-views
issue:
pr:
mod-block: merge:pr-merge
---

Implement phase P3 of the SpaceTop v2 internals rebuild: add read-only TUI
capability views on top of the P1/P2 query API, including search, command
palette, timeline, metrics, activity feed, and typed entity details.

Plan: `docs/superpowers/plans/2026-06-11-spacetop-v2-p3-capability-views.md`

This entity was fired without dispatch at the captain's request.

## Scope

- Kind: feature
- Risk: medium
- Milestone: v2-p3
- Touches: app modes, input handling, Ratatui views, footer/help, core relation/detail view models
- Non-goals: workflow writes, new history derivation, config/keybinding persistence, headless CLI

## Acceptance criteria

**AC-1 -- Search and command palette are operable.**
Users can type, backspace, move selection, press Enter, and close with Esc; commands open the expected read-only views.
Verified by: app input tests and search overlay render tests.

**AC-2 -- Timeline, metrics, and activity consume core APIs only.**
Views render populated data, loading state, unavailable state, and empty state without reaching around `WorkflowIndex`.
Verified by: Ratatui view tests and core DTO tests.

**AC-3 -- Entity details are typed in core.**
Issue, PR, feedback stage, and worktree provenance facts are exposed through core detail/relation DTOs, not inferred in UI code.
Verified by: core relation tests and relations view tests.

**AC-4 -- Existing app plumbing survives every new mode.**
Reload, sync status, help/footer, repo-root access, and workflow switching keep working while P3 views are open.
Verified by: app mode regression tests.

## Proof plan

- Lowest test layer: core relation tests, app input/mode tests, then Ratatui render tests.
- Required commands: `cargo test --workspace`; `make lint`; `cargo test -p spacetop-core --test no_terminal_deps`.
- Manual check, if any: optional TUI smoke for key ergonomics.
- Docs/policy update needed: README/help/footer keybinding documentation.

## Stage Report: implement

- DONE: Add read-only capability views and input flows for search, command palette, timeline, metrics, activity, and entity details, preserving existing archive/sort/preview/sync/workflow-switch behavior.
  Implemented in commits 7345d07 and 74b0a73; covered by app mode tests, app key tests, and Ratatui render tests.
- DONE: Keep workflow facts typed before rendering: add core relation/detail DTOs and have timeline, metrics, activity, and details views consume WorkflowIndex/core APIs without schema inference in UI code.
  Core DTOs and index methods are in 6d339a2; renderers call WorkflowIndex query, timeline, metrics, activity, and entity_details APIs.
- DONE: Cover the new modes at the lowest practical layer with core, app/input, and Ratatui rendering tests, then run cargo test --workspace, make lint, and cargo test -p spacetop-core --test no_terminal_deps.
  Passed `cargo test --workspace`, `make lint`, and `cargo test -p spacetop-core --test no_terminal_deps` on this worktree.

### Summary

Implemented P3 capability views as read-only TUI surfaces backed by core query, history, metrics, activity, and relation/detail APIs. Updated footer, help, and README documentation for the new keybindings, with focused core, app, and Ratatui coverage plus the required full verification commands passing.
