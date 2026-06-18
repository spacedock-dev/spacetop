---
title: Refine session preview wording
status: shape
source: "Follow-up from task 067 on 2026-06-18: preview metadata wording is too implementation-heavy"
kind: feature
risk: low
milestone: v1-maintenance
proof: Ratatui preview rendering tests covering the revised session metadata line
started:
completed:
verdict:
score: 0.78
worktree:
issue:
pr:
id: 068
---

Refine the selected-task preview metadata for agent session attribution. The current wording exposes implementation details such as `session: xxx <effort>` and `via: <reason>` in the preview header, which is useful for debugging but too noisy for ordinary inspection.

Acceptance criteria:

- AC-1: The preview no longer renders `via: <reason>` in the normal selected-task header.
- AC-2: The session label wording is clearer than `session: xxx <effort>` while still preserving useful agent/session confidence information somewhere appropriate.
- AC-3: Task-list active marker behavior remains unchanged.
- AC-4: Ratatui preview tests cover the revised wording and ensure the removed `via:` label does not reappear.
- AC-5: Verification includes `cargo test -p spacetop` for relevant UI tests and `make lint`, or records blockers.
