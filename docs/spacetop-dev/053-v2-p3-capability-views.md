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
pr: "#53"
mod-block:
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

## Stage Report: verify

- FAILED: Independently validate AC-1 and AC-4: search/command palette input works, new modes close/route correctly, and reload/sync/help/footer/workflow switching still behave while P3 views are open.
  `cargo test --workspace` covers search/command routing, but full-pane P3 modes do not handle `?`, `Left`, `Right`, or `P`; tests do not prove those AC-4 paths work while views are open.
- DONE: Validate AC-2 and AC-3 from code and tests: timeline, metrics, activity, and details consume WorkflowIndex/core relation APIs and do not infer workflow schema facts in UI code.
  Renderers call `WorkflowIndex::timeline`, `metrics`, `activity`, and `entity_details`; core relation and Ratatui view tests passed in `cargo test --workspace`.
- DONE: Run or audit required proof commands for P3: cargo test --workspace, make lint, and cargo test -p spacetop-core --test no_terminal_deps; report PASS/FAIL and any blocking defects.
  PASS: `cargo test --workspace`, `make lint`, and `cargo test -p spacetop-core --test no_terminal_deps` all exited 0.

### Summary

Verification rejects the implementation as incomplete on AC-4. Search, command routing, core-backed timeline/metrics/activity/details, docs, lint, and no-terminal-dependency proof all pass, but help and workflow-switch behavior is not available or evidenced while the full-pane P3 views are open.

### Feedback Cycles

- Cycle 1: Verify rejected AC-4 coverage because full-pane P3 modes do not handle `?`, `Left`, `Right`, or `P`; add behavior and tests proving help, workflow switching, and picker access still work while timeline, metrics, activity, and details views are open.

## Stage Report: implement (cycle 2)

- DONE: Full-pane P3 modes (timeline, metrics, activity, entity details/relations) must preserve existing app plumbing while open.
  Commit f90c8bd routes full-pane P3 modes through session-backed help, switching, picker, and materialization paths.
- DONE: Specifically, handle `?` for help, `Left`/`Right` for workflow switching in multi-session mode, and `P` for picker access while these full-pane views are open.
  `p3_full_pane_modes_open_help_and_esc_still_restores_overview`, `p3_full_pane_modes_switch_workflows_left_and_right`, and `p3_full_pane_modes_can_open_picker_overlay` passed.
- DONE: Add regression tests proving those keys work from full-pane P3 views, not only from overview/search/command modes.
  Added focused app/input coverage for all four full-pane modes: `T`, `M`, `A`, and `R`.
- DONE: Preserve existing Esc behavior back to overview and do not regress archive/sort/preview/sync behavior.
  Esc behavior is asserted in the new help test; `cargo test --workspace` passed the existing archive, sort, preview, and sync suites.
- DONE: Commit the fix and append a new implement/fix stage report or update the existing task record with concise evidence and commands run.
  Fix commit: f90c8bd. Commands run: `cargo test -p spacetop p3_full_pane_modes`, `cargo test --workspace`, and `make lint`.

### Summary

Fixed the AC-4 rejection by letting full-pane P3 views handle help, multi-workflow switching, picker access, and lazy active-workflow materialization through the same app plumbing used by overview. The focused P3 full-pane regression tests, full workspace test suite, and lint gate all pass.

## Stage Report: verify (cycle 2)

- DONE: Verify the cycle-1 AC-4 fix: full-pane P3 modes handle ? help, Left/Right workflow switching, P picker access, Esc back to overview, and do not regress existing archive/sort/preview/sync behavior.
  PASS: `cargo test -p spacetop p3_full_pane_modes` passed 3/3; `cargo test --workspace` also passed existing archive, sort, preview, and sync suites.
- DONE: Re-check AC-1 through AC-3 after the fix: search/command palette remain operable, timeline/metrics/activity consume WorkflowIndex/core APIs, and entity details stay typed in core.
  PASS: app/search and UI tests passed; renderers call `WorkflowIndex::timeline`, `metrics`, `activity`, and `entity_details`, with relation DTOs owned by core.
- DONE: Run or audit required proof commands after the fix: cargo test --workspace, make lint, and cargo test -p spacetop-core --test no_terminal_deps; report PASS/FAIL and any remaining blocker.
  PASS: `cargo test --workspace`, `make lint`, and `cargo test -p spacetop-core --test no_terminal_deps` all exited 0; no remaining blocker found.

### Summary

Verification accepts the cycle-2 AC-4 fix. Full-pane P3 modes now preserve help, workflow switching, picker access, Esc restoration, and existing app plumbing while AC-1 through AC-3 remain covered by core, app, and Ratatui tests.
