---
title: Fix unrelated session running attribution
status: shape
source: "Follow-up from 2026-06-18 screenshot: newly created task 068 is shown running from an unrelated dataagentbench Codex session"
kind: bug
risk: medium
milestone: v1-maintenance
proof: Focused session-activity tests with unrelated workspace/session fixtures plus a preview/task-list rendering regression
score: 0.86
id: 070
---

# Fix unrelated session running attribution

Spacetop can still mark a newly created Spacedock task as `running` by attaching an unrelated agent session from another workspace. The observed false positive is task `068` in `docs/spacetop-dev`, where the preview shows a Codex session rooted under `dataagentbench` and reports `state: running` even though that session has no relationship to the Spacetop workflow task.

## Acceptance criteria

- **AC-1:** Running/recent/stale/human-gated evidence must be scoped to a session that is actually related to the workflow task, not just a loose numeric or text match.
- **AC-2:** A Codex or Claude session rooted in an unrelated workspace, such as `dataagentbench`, must not mark a Spacetop task as `running` merely because the transcript contains broad matching text.
- **AC-3:** The session attribution path should make the accepted relationship explicit in code: task id, slug, workflow path, worktree path, or other deliberate evidence.
- **AC-4:** Tests must cover the screenshot-shaped false positive: a new task with an unrelated active session should render as non-running.
- **AC-5:** Verification includes focused core tests for session attribution and the relevant TUI rendering test, plus `make lint`.
