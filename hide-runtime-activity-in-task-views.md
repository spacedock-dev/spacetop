---
title: Hide runtime activity from task list and detail header
status: plan
source: Captain request to remove unreliable per-task agent status, 2026-10-02
kind: feature
risk: low
milestone: v1-maintenance
proof: Ratatui TestBackend assertions for list and preview metadata across runtime states, scopes and terminal widths; workflow stages and durable gates remain visible.
started: 2026-10-02T02:11:23Z
completed:
verdict:
worktree:
issue:
pr:
mod-block:
id: 084
---

Make task browsing clearer by removing inferred agent runtime activity from the task list and detail header. The captain finds Idle unhelpful and real agent status difficult to determine for an individual task, and prefers those surfaces not to suggest a live activity state.

## Scope

- Remove runtime activity labels (Idle, Running and HumanGate, including Worker/FirstOfficer variants) and activity-only markers/colors from task-list rows.
- Remove the runtime/session/activity-status/updated attribution line from the detail/preview header so stale inference does not appear as current information.
- Keep workflow stage/status, score, source, worktree markers, archived verdicts and durable gate readiness/details visible; those are workflow facts, separate from inferred session activity.
- Touches: crates/spacetop/src/ui/list.rs, ui/preview.rs, nearby rendering tests and affected docs.
- Non-goals: Removing backend activity detection, changing headless activity/export schemas, removing the dedicated activity feed, redesigning workflow stages, changing pending task 082, or writing workflow state from Spacetop.

## Acceptance criteria

**AC-1:** Active and archived task rows show no runtime activity text or activity-only running/human-gate glyphs/colors for Idle, Running(Worker), Running(FirstOfficer), HumanGate or missing activity. Workflow stage, selection, worktree marker, title and durable gate indicator remain correct.

**AC-2:** Selected-task detail/preview header shows no runtime/session/activity-status/last-activity timestamp line across those states. Workflow status/score/source and durable gate facts remain available, and body content containing similar words is preserved.

**AC-3:** Ratatui TestBackend tests cover narrow/wide layouts, active/archive scopes and all runtime variants. Removing the suppression or accidentally removing workflow-stage/gate metadata must fail the assertions. Update user-facing docs and obsolete positive activity-display tests together.

**AC-4:** The change remains confined to these display surfaces and preserves keyboard behavior, read-only safety, core activity data and headless contracts. Run cargo fmt --check, cargo test and make lint before completion.

## Proof plan

Use existing ui/tests/task_list.rs and ui/tests/preview.rs plus shared runtime fixtures at the lowest rendering layer. Assert the rendered headers/rows rather than globally excluding words from user-authored markdown bodies. Keep durable gate assertions and ASCII/worktree/selection layout checks. No real watcher rerun is required unless watcher behavior changes.
