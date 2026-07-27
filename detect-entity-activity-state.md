---
title: Detect entity activity and human-gated sessions
status: shape
source: "Follow-up from task 067 and the 2026-07-27 three-state activity design refinement"
kind: feature
risk: medium
milestone: v1-maintenance
proof: Core activity-state tests with representative Codex and Claude session logs plus task-list rendering tests for status and handler
started:
completed:
verdict:
score: 0.84
worktree:
issue:
pr:
id: 069
---

Show one concise activity status for each entity:

- `idle`: no observable worker or first-officer activity is handling the entity.
- `running`: an observable worker or first-officer action is handling the entity.
- `human-gate`: the first officer is waiting for a human to approve or reject.

`running` also carries a handler, rendered inside the status as
`running · worker` or `running · FO`. There is no separate handler column, and
the handler is not a fourth status. The other statuses render as `idle` and
`human-gate` without a handler suffix. `idle` describes agent activity, not
workflow completion; an entity can be unfinished while idle.

The intended lifecycle includes both first-officer windows:

```text
idle
  -> running · FO
  -> running · worker
  -> running · FO
  -> idle | human-gate | running · worker
```

The first `running · FO` covers preparing and dispatching the assignment. The
second covers the handoff window after a worker finishes, while the first
officer processes the result or updates the entity before dispatching another
worker.

Detection must work from existing local Codex and Claude session artifacts and
workflow filesystem events. It must not require a new Spacedock plugin hook or
assume that `spacedock status --boot --json` knows runtime state.

Observable evidence includes:

- A worker starts when a worker session accepts the entity's exact dispatch
  assignment.
- A worker stops handling the entity when its structured terminal/idle event is
  observed, such as Codex `task_complete` or Claude `idle_notification`.
- First-officer work is observable through a structured FO tool call scoped to
  the exact entity, including reading or updating its file, building its
  dispatch assignment, or spawning its worker. A matching filesystem change may
  support that evidence but must not by itself claim FO identity.
- A first-officer turn completion ends `running · FO` unless a worker has
  started or a human gate is pending.
- Silent reasoning before an observable action is not detectable and must not
  be guessed as running.

When evidence overlaps, display precedence is:

```text
human-gate > running · worker > running · FO > idle
```

The task-list marker should render `human-gate` with red or equivalent
high-salience styling. Running entities should identify their handler without
adding more status values.

The existing session-evidence fields should be simplified for this model:

- `Agent` becomes `Runtime` and continues to identify Codex or Claude.
- `Session` remains and identifies the currently relevant worker or FO session.
  It is empty for `idle`; historical sessions may remain available in details
  but must not look like current handlers.
- `Confidence` is removed from the user-facing activity display. Recognized
  structured events determine the state; scanner or parser uncertainty is
  reported separately rather than becoming another activity status.
- `Status` renders exactly `idle`, `running · worker`, `running · FO`, or
  `human-gate`, backed by only the three domain status values.
- `Latest` becomes `Updated`, meaning the time of the latest relevant activity
  or state-transition event.

Acceptance criteria:

- AC-1: The visible domain status has exactly three values: `idle`, `running`,
  and `human-gate`.
- AC-2: `running` carries a typed `worker` or `FO` handler and renders it inside
  the status as `running · worker` or `running · FO`; there is no separate
  handler column and the other statuses have no handler suffix.
- AC-3: Initial state and any state with no observable handler or pending human
  decision classify as `idle`.
- AC-4: A worker session accepting the entity's exact dispatch assignment
  classifies as `running · worker` until a structured terminal/idle event.
- AC-5: Observable FO actions scoped to the exact entity classify as
  `running · FO` before dispatch and during the worker-to-FO handoff window.
- AC-6: The shape or plan pins concrete Codex and Claude start, terminal/idle,
  FO-turn, and human-gate record patterns without leaking transcript content.
- AC-7: Detection works without changing the Spacedock plugin. General path
  mentions, process names, dispatch-file existence, and filesystem writes alone
  are insufficient to claim a handler.
- AC-8: `human-gate` requires evidence that the FO is waiting for an
  approve/reject decision; merely mentioning approval or occupying a
  gate-marked workflow stage is insufficient.
- AC-9: Rendering follows
  `human-gate > running · worker > running · FO > idle`, uses high-salience
  styling for `human-gate`, and exposes the compact fields `Runtime`,
  `Session`, `Status`, and `Updated` without a user-facing `Confidence` or
  `Handler` field.
- AC-10: Tests cover the normal lifecycle, pre-dispatch FO activity,
  worker-to-FO handoff, immediate next-worker dispatch, overlapping evidence,
  false-positive mentions, and unavailable or malformed session artifacts.
- AC-11: Verification includes focused core tests, rendering tests, and
  `make lint`, or records blockers.
