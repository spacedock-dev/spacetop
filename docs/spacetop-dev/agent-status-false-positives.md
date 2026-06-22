---
title: Agent status false positives on undispatched tasks
status: plan
source: captain report (session)
kind: bugfix
risk: medium
id: 072
started: 2026-06-22T05:14:50Z
---

Agent status detection produces many false positives: it reports a running/active
agent state for tasks that have NOT been dispatched. A task that has never had a
worker dispatched should never show as "running".

Likely related to task 069 (detect human-gated agent sessions) — the running-state
heuristic is too broad and keys off signals that are present even for undispatched
tasks.

Shape should pin down: what signal currently drives the "running" determination,
why it fires for undispatched tasks, and the correct condition (e.g. require an
active worktree / live worker handle / dispatch marker before reporting running).

## Shape

### Problem statement

Spacetop's task list shows the active-session marker on tasks that have never had
a worker dispatched. A captain scanning the list cannot trust the marker: tasks
sitting in `backlog`/`shape` with no worktree and no dispatched worker still light
up as if an agent is running on them, drowning out the handful of tasks that are
actually in flight.

### Root cause (pinned)

The active marker is `EntitySessionAttribution::has_active_marker()`
(`crates/spacetop-core/src/domain/mod.rs:319`), which is true when any evidence
item is `is_active_marker()` — `confidence >= Medium && run_state == Running`
(`domain/mod.rs:265`). Both halves of that conjunction fire for undispatched
tasks:

1. **The match half has no dispatch requirement.** `match_entity`
   (`crates/spacetop-core/src/session_activity.rs:409`) attributes a session to
   an entity at `Medium` confidence whenever the session content merely *mentions
   the task file path* via `explicit_entity_reference_paths`
   (`session_activity.rs:440-443`, `:490`). It also matches `High` on a worktree
   path mention. Nothing requires that the session was *dispatched for that task*.
   The only dispatch-awareness in the file is the *negative* guard
   `has_conflicting_dispatch_assignment` (`session_activity.rs:451`), which
   excludes a session only when it carries a
   `/tmp/spacedock-dispatch/spacedock-ensign-<slug>-<stage>.md` marker for a
   *different* slug. A session with **no** dispatch marker at all — the captain's
   or first officer's own orchestration session — sails through the filter.

2. **The liveness half fires on the orchestrator's own process.** `classify`
   (`session_activity.rs:369`) returns `Running` for `LivePid`,
   `LiveResumeCommand`, or `ObservedSessionWrite` (`domain/mod.rs:281-289`). The
   captain/FO session is a genuinely live process (real `pid`, or actively
   writing its own session log), so it scores `Running` on its own merits.

Combined: a single live orchestration session that references an undispatched
task's file path (during triage, shaping, `status`/listing, or because the FO
wrote the task) yields `Medium` + `Running` -> `has_active_marker() == true`.
The signal that *should* gate "running" — evidence that a worker was actually
dispatched to work this entity (a positive dispatch marker, or a match anchored
to a live worktree the entity owns) — is never required.

This is exactly the over-broad heuristic flagged in task 069: liveness is keyed
off process/file-write signals that are present for the orchestrator, not just
for dispatched workers.

The TUI is **not** at fault. `crates/spacetop/src/ui/list.rs:178-179` renders the
marker straight from `index().entity_has_active_session_marker(id)`, which
forwards `has_active_marker()`. It faithfully shows whatever the domain reports;
the fix belongs in the core classifier/matcher, not in marker rendering.

### Target outcome

A captain trusts the active marker: it appears only for tasks with a live,
dispatched worker (or an equivalently strong, dispatch-anchored liveness signal).
A never-dispatched task classifies as not-running and shows no active marker, even
when a live orchestrator session references it.

### Acceptance criteria

- AC-1: The domain distinguishes "a session is alive" from "a dispatched worker
  is alive for this entity." Only the latter may drive the active marker. The
  positive dispatch/ownership signal is named in the model, not inferred ad hoc
  in UI or call sites.
- AC-2 (negative case — primary): A never-dispatched task classifies as
  not-running. Concrete false-positive example to drive a test: a single live
  Claude/Codex session (real running `pid`) whose content references the task
  file path `docs/spacetop-dev/agent-status-false-positives.md` but carries **no**
  `/tmp/spacedock-dispatch/spacedock-ensign-...` marker for this entity. Today
  this produces `has_active_marker() == true`; after the fix it must produce
  `has_active_marker() == false` (the entity may still appear as recent, but not
  active).
- AC-3 (positive case preserved): A task whose live session carries a matching
  dispatch marker, or whose live session is anchored to the entity's own
  worktree, still classifies as running and keeps the active marker. The existing
  worktree/resume-command running tests in `session_activity.rs` must keep
  passing.
- AC-4: No reliance on machine-specific paths or process names beyond the
  existing probe abstraction; behavior is testable through `ProcessProbe` and
  fixture session files.

### Scope boundaries

- In scope: the core session-activity matcher/classifier and the domain model
  that separates session liveness from dispatched-worker liveness for an entity.
- Out of scope: TUI marker styling/rendering (it already forwards the domain
  decision); the human-gated state work in task 069 (related but separate
  marker); any write path or git behavior.
- Non-goal: removing recent/stale classification — those are not the bug; only
  the over-broad *active/running* marker is.

### Product contract touched

The **core session-activity / liveness domain model** (`spacetop-core`:
`session_activity.rs` matcher/classifier and `domain/mod.rs`
`AgentSessionLiveness`/`EntitySessionAttribution`). Per the Code Map, TUI must
consume this through typed domain state and must not infer schema rules from
strings — so the correct fix layer is the domain, with parser/core tests as the
lowest practical proof. TUI marker rendering is downstream and unchanged.

### Risk and milestone

- Risk: medium (matches frontmatter). It narrows an existing classification; the
  failure mode of getting it wrong is a missed active marker, not data loss or a
  write, and the read-only contract is untouched.
- Milestone: v1-maintenance (consistent with the related task 069).

## Stage Report: shape

- DONE: Pin the root cause: identify the exact signal that currently classifies a task/agent as running or active, and explain why it fires for tasks that were never dispatched
  Pinned to `has_active_marker()` = Medium-confidence path/worktree mention (`session_activity.rs:409`/`:490`) AND `Running` liveness from the orchestrator's own live process (`classify` `session_activity.rs:369`); no positive dispatch requirement, only the negative `has_conflicting_dispatch_assignment` guard (`session_activity.rs:451`).
- DONE: Define acceptance criteria that include the negative case — a never-dispatched task must classify as not-running — with at least one concrete false-positive example to drive a test
  AC-2 specifies a live session referencing this entity's file path with no dispatch marker -> must yield `has_active_marker() == false`; AC-3 preserves the dispatched/worktree-anchored positive cases.
- DONE: Name the product contract touched (session-activity/liveness domain model vs TUI marker rendering), and confirm risk level and milestone
  Contract is the core session-activity/liveness domain model (`spacetop-core`); confirmed TUI (`ui/list.rs:178`) only forwards the domain decision. Risk medium (matches frontmatter), milestone v1-maintenance.

### Summary

Root-caused the false positive to the core classifier: the active marker requires
only a Medium path-mention match plus any Running liveness, and the orchestrator's
own live session satisfies both for undispatched tasks. The matcher has no
positive dispatch requirement — only a negative guard against conflicting
dispatch slugs. Defined a negative-case acceptance criterion with a concrete
false-positive example, kept the positive dispatched/worktree cases in scope, and
located the fix in the session-activity/liveness domain model rather than TUI
rendering. Risk medium, milestone v1-maintenance.
