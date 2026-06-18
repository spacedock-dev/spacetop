---
title: Detect human-gated agent sessions
status: shape
source: "Follow-up from task 067 on 2026-06-18: Spacetop should distinguish agents waiting on human approval"
kind: feature
risk: medium
milestone: v1-maintenance
proof: Core session-activity tests with representative Codex and Claude session log snippets plus task-list rendering tests for the gated marker
started:
completed:
verdict:
score: 0.84
worktree:
issue:
pr:
id: 069
---

Detect when an attributed agent session is blocked on human input, such as a first officer asking the captain to approve or reject a gate, and surface that state distinctly from ordinary running/recent/stale activity.

The task-list marker should visually distinguish this state, with a red marker or equivalent high-salience styling, so a captain can quickly see which tasks need human action.

Acceptance criteria:

- AC-1: The shape or plan documents concrete Codex and Claude session-log patterns that indicate a human gate without leaking transcript content.
- AC-2: The domain model separates human-gated evidence from running/recent/stale liveness so marker policy stays explicit.
- AC-3: Spacetop can classify an attributed session as human-gated from session artifacts without relying on process-name-only matching.
- AC-4: The task list renders a distinct red or equivalent urgent marker for human-gated tasks.
- AC-5: Preview or detail text uses concise wording such as `human gated` or a better agreed label.
- AC-6: Tests cover positive human-gated examples and false positives where a transcript merely mentions approval but is not waiting for the user.
- AC-7: Verification includes focused core tests, rendering tests, and `make lint`, or records blockers.
