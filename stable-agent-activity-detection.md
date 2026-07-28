---
title: Fix intermittent Codex and Claude activity detection
status: plan
source: "Live false-idle observation while dispatching task 074 on 2026-07-28"
kind: bugfix
risk: high
milestone: v1-maintenance
proof: Reproducing Codex and Claude session fixtures plus repeated scanner and task-list activity tests
started: 2026-07-28T06:55:46Z
completed:
verdict:
score: 0.92
worktree:
issue:
pr:
id: 075
---

Spacetop does not reliably detect active task handling from Codex and Claude Code session logs. The same dispatched worker can appear as `running · worker` in one scan and `idle` in another, or remain `idle` while it is actively processing the task.

A live example occurred while task 074 was in `plan`: the Codex planning worker `/root/spacedock_ensign_compact_copyable_slug_ids_plan` was active, but Spacetop displayed the task as `idle`. Similar intermittent false-idle behavior has been observed with Claude Code workers.

The activity display must remain stable while structured evidence says a worker or first officer is handling the task. It should return to `idle` only after the corresponding structured terminal or idle event, subject to the existing `human-gate > running · worker > running · FO > idle` precedence.

## Acceptance criteria

- **AC-1:** An exact dispatched Codex worker remains `running · worker` across repeated scans until its structured completion or idle event is observed.
- **AC-2:** An exact dispatched Claude Code worker remains `running · worker` across repeated scans until its structured terminal or idle event is observed.
- **AC-3:** The investigation reproduces the task 074 false-idle sequence or an equivalent sanitized session sequence and identifies which evidence is missed, dropped, or incorrectly invalidated.
- **AC-4:** Partial appends, delayed records, unchanged scans, cursor reuse, log rotation or truncation, and session-file ordering cannot transiently erase still-valid active evidence for either runtime.
- **AC-5:** Worker completion and first-officer handoff still transition correctly; the fix must not leave completed workers stuck as running or weaken exact task attribution and false-positive rejection.
- **AC-6:** Tests cover repeated scans and lifecycle transitions for both Codex and Claude Code, including an active-worker false-idle regression and task-list rendering of the resulting status.
- **AC-7:** Verification includes focused scanner tests, full `cargo test`, `make lint`, and a live or fixture replay showing stable status over multiple scan intervals.

## Investigation context

The defect is intermittent and affects the structured session-activity scanner rather than workflow frontmatter. Inspect session snapshot invalidation, incremental cursor summaries, record ordering, parent linkage, and reducer state retention before changing UI rendering.
