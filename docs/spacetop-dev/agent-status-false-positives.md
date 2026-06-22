---
title: Agent status false positives on undispatched tasks
status: implement
source: captain report (session)
kind: bugfix
risk: medium
id: 072
started: 2026-06-22T05:14:50Z
worktree: .worktrees/spacedock-ensign-agent-status-false-positives
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

## Plan

### Owned files / modules

- `crates/spacetop-core/src/session_activity.rs` — matcher + classifier (primary change).
- `crates/spacetop-core/src/domain/mod.rs` — `AgentSessionLiveness` /
  `AgentSessionEvidence` / `EntitySessionAttribution` (model + marker predicate).
- Tests in `crates/spacetop-core/src/session_activity.rs` (`#[cfg(test)] mod tests`)
  — primary proof layer.
- `AGENTS.md` Code Map / `## Current Product Shape` — one-line behavior note for
  the narrowed marker rule (docs in the same change).

### Design decision: what gates the active marker

The active marker is `is_active_marker()` = `confidence >= Medium && run_state ==
Running` (`domain/mod.rs:265`). The bug is that this never checks whether the
matched session is a *dispatched worker for this entity*. The fix adds a positive
dispatched-worker / ownership requirement.

Two candidate liveness signals already encode strong ownership and should remain
marker-eligible; the weak ones should not, on their own, drive the marker:

- `LivePid` / `LiveResumeCommand` / `ObservedSessionWrite` from a session that
  matched at `High` confidence via the entity's **own worktree path**
  (`match_entity` worktree branch, `session_activity.rs:427-438`) — this is an
  ownership-anchored running signal. KEEP marker-eligible.
- A session carrying a positive `/tmp/spacedock-dispatch/spacedock-ensign-<slug>-
  <stage>.md` marker **for this entity's slug** — explicit dispatch evidence.
  KEEP marker-eligible. (Today `dispatch_assignment_slugs` is read only to
  *reject* conflicting slugs; the plan also reads it as positive confirmation.)
- A `Medium`-confidence match (bare task-file-path mention via
  `explicit_entity_reference_paths`) with any Running liveness — this is the
  orchestrator false positive. NOT marker-eligible on its own.

Mechanism: introduce a typed notion of dispatch/ownership on the evidence so the
marker predicate reads from typed state, not re-derived strings (Domain-before-UI,
Code Map "consume through query methods"). Concretely, add a boolean-like typed
field to `AgentSessionEvidence` — e.g. `dispatch_anchor: DispatchAnchor` with
variants `OwnedWorktree` / `DispatchedForEntity` / `None` — set in
`scan_local_sessions_inner` from (a) the worktree-confidence match and (b) a new
positive `dispatch_assignment_slugs(content)` check against this entity's slug.
Then `is_active_marker()` becomes:
`run_state == Running && dispatch_anchor != None` (drop the bare `confidence >=
Medium` gate as the sole qualifier). This keeps the predicate a pure function of
typed evidence and removes the orchestrator path from the active set while leaving
`run_state`/`confidence` reporting intact for preview/detail text.

### Step-by-step

1. **Add the typed ownership signal to the domain.** In `domain/mod.rs`, add
   `DispatchAnchor` enum and a `dispatch_anchor` field on `AgentSessionEvidence`;
   update `is_active_marker()` to require `Running && dispatch_anchor !=
   DispatchAnchor::None`. Keep `run_state()`, `confidence`, and `best_evidence()`
   unchanged so recent/stale reporting and preview text are unaffected.
2. **Populate the anchor in the scanner.** In `session_activity.rs`
   `scan_local_sessions_inner`, compute the anchor per (entity, session): set
   `OwnedWorktree` when `match_entity` returned via the worktree branch (thread a
   small enum/flag out of `match_entity` instead of today's
   `Option<PathBuf>`-only return, or compute alongside it); set
   `DispatchedForEntity` when `dispatch_assignment_slugs(content)` contains this
   entity's `entity_slug`. Construct `AgentSessionEvidence` with that anchor.
3. **Add a positive dispatch-slug check** reusing `dispatch_assignment_slugs` and
   `entity_slug` (already present) — no new parsing surface.
4. **Run the lowest-layer proof** (tests below).
5. **Update docs** (`AGENTS.md` behavior note) in the same change.

### Lowest-practical-layer proof

Core `session_activity` tests (`#[cfg(test)] mod tests`), fixture-driven via
`FixtureProbe` / `CommandProbe`:

- **AC-2 regression (new, primary):** live session (`pid` in `FixtureProbe`)
  whose content references `docs/spacetop-dev/agent-status-false-positives.md`
  (Medium match) with **no** dispatch marker → assert
  `run_state() == Running` (still alive) but
  `!attribution.has_active_marker()`. This is the test that fails today and must
  pass after the change.
- **AC-3 positive cases (must keep passing):** the existing
  `running_codex_worktree_match_is_high_confidence_active`,
  `running_claude_code_worktree_match_is_high_confidence_active`,
  `pidless_codex_resume_command_marks_session_running`, and
  `pidless_claude_resume_command_marks_session_running` tests — worktree-anchored
  and resume-command running sessions stay `has_active_marker()`.
- **New positive dispatch-marker test:** a live session carrying
  `/tmp/spacedock-dispatch/spacedock-ensign-<this-slug>-implement.md` and a
  Medium-only match → asserts `has_active_marker()` (dispatch anchor qualifies a
  non-worktree task).
- **Marker rendering:** existing `crates/spacetop/src/ui/tests/task_list.rs`
  marker test continues to assert the green dot for a Running attribution; it
  drives `has_active_marker()` through `entity_has_active_session_marker`, so no
  TUI logic change is needed. If the test fixture builds evidence directly, set
  its `dispatch_anchor` so it stays marker-eligible.

Note on the existing `mtime_only_recent_match_does_not_mark_active` and
`pidless_match_without_live_session_command_falls_back_to_stale` tests: these
already assert no active marker for non-Running states and remain valid.

### Verification commands

- `cargo fmt`
- `cargo test` (full workspace; covers the new core tests and existing UI tests)
- `cargo test -p spacetop-core session_activity` (focused fast loop during dev)
- `make lint` (`cargo clippy --all-targets --all-features -- -D warnings`) —
  required completion gate.

### Docs / policy updates in the same change

- `AGENTS.md`: update the session-activity behavior note (under Code Map for
  `session_activity.rs` / `domain/mod.rs`) to state the active marker now requires
  a dispatched-worker or own-worktree anchor, not a bare path mention. No policy
  rule changes (read-only/git/config contracts untouched). No README user-facing
  command change (the marker is described, not commanded).

### Spike

No spike needed. The proving mechanism — fixture session files + `ProcessProbe`
fakes driving `scan_local_sessions_with` and asserting `has_active_marker()` /
`run_state()` — is already established by the existing tests in the same module
(`session_activity.rs:767-1572`). The change reuses existing helpers
(`dispatch_assignment_slugs`, `entity_slug`, `match_entity`) and adds one typed
field plus one predicate condition.

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

## Stage Report: plan

- DONE: Produce a step-by-step implementation plan naming the owned files/modules (`session_activity.rs` matcher/classifier, `domain/mod.rs` liveness/attribution) and the exact change that makes the active marker require a positive dispatched-worker/ownership signal — not just a Medium path-mention plus any Running liveness
  Plan adds a typed `DispatchAnchor` field on `AgentSessionEvidence` set in `scan_local_sessions_inner` from the worktree-confidence match and a positive `dispatch_assignment_slugs` check; `is_active_marker()` becomes `Running && dispatch_anchor != None` (`domain/mod.rs:265`, `session_activity.rs:409`/`:451`).
- DONE: Specify the lowest-practical-layer proof: core session-activity tests with a fixture reproducing the AC-2 false positive (live session referencing the task path, no dispatch marker → not active) and the AC-3 positive cases (dispatch-marker / worktree-anchored) still passing; name exact verification commands (`cargo test`, `make lint`)
  Named the new AC-2 regression test, the four existing AC-3 worktree/resume tests that must keep passing, and a new positive dispatch-marker test; verification: `cargo fmt`, `cargo test`, `cargo test -p spacetop-core session_activity`, `make lint`.
- DONE: Identify any docs/policy updates needed in the same change (Code Map / behavior notes) and state whether a spike is needed before the matcher change, or record "no spike needed" with the proven mechanism
  `AGENTS.md` session-activity behavior note updated in the same change; no policy/README command change; recorded "no spike needed" — fixture + `ProcessProbe` mechanism is already proven by existing tests in `session_activity.rs`.

### Summary

Planned a domain-first fix: introduce a typed `DispatchAnchor` on session
evidence so the active marker requires either an own-worktree-anchored or a
positively-dispatched running session, dropping the bare Medium-path-mention path
that lets the orchestrator's live session light up undispatched tasks. The proof
is a new AC-2 core regression test plus preservation of the existing
worktree/resume running tests and a new positive dispatch-marker test, verified
with `cargo test` and `make lint`. No spike needed; the mechanism reuses existing
helpers and fixture patterns. One `AGENTS.md` behavior note ships in the same
change; read-only/git/config contracts are untouched.
