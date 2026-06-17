---
title: Agent session active marker
status: plan
source: docs/spacetop-dev/_artifacts/2026-06-14-agent-session-progress-survey.md; https://github.com/spacedock-dev/spacetop/issues/28
kind: feature
risk: medium
milestone: v2-later
proof: scanner fixtures, app result application tests, Ratatui task-row rendering tests, make lint
started: 2026-06-17T03:45:57Z
completed:
verdict:
score: 0.84
worktree:
issue: https://github.com/spacedock-dev/spacetop/issues/28
pr:
id: 065
---

Implement the narrow first slice from the 2026-06-14 agent session progress survey: show whether an active task appears to be handled by a currently running matched Codex or Claude Code session.

The first slice should stay local, read-only, and confidence-rated. It should answer the scanning question "is this task actively being handled right now?" without claiming authoritative ownership, lock state, waiting-for-user, or stuck/progress states.

## Scope

- Kind: feature
- Risk: medium
- Milestone: v2-later
- Touches: parser / app-state / UI / docs
- Non-goals: authoritative assignment metadata, waiting/stuck state inference, workflow markdown writes, transcript rendering, remote upload, Nerd Font-only UI

## Acceptance criteria

**AC-1 -- Running matched Codex sessions are visible in task rows.**
A task with high-confidence Codex worktree evidence and a running matched session renders a compact active marker in the task list.
Verified by: core scanner/correlation fixtures plus Ratatui task-row rendering tests.

**AC-2 -- Running matched Claude Code sessions are visible in task rows.**
A task with high-confidence Claude Code worktree evidence and a running matched session renders the same compact active marker in the task list.
Verified by: core scanner/correlation fixtures plus Ratatui task-row rendering tests.

**AC-3 -- Old or weak evidence does not mark active work.**
A task without a running matched session renders no active marker, even if old or low-confidence session evidence exists.
Verified by: correlation ranking tests covering stale, recently active, and low-confidence evidence.

**AC-4 -- Preview details identify the matched agent without leaking transcript content.**
When attribution exists, the preview/details area names Codex, Claude Code, or multi-agent, plus session identity or display name, confidence, run state, and latest activity; it does not render prompt, response, command output, or transcript body text.
Verified by: rendering tests and fixture assertions over derived structural evidence only.

**AC-5 -- Workflow loading remains resilient and read-only.**
Session scanner failures are non-fatal, workflow markdown is not modified, and `spacetop-core` remains terminal-free.
Verified by: scanner failure tests, existing no-terminal-deps guardrail, no workflow-write implementation path, and `make lint`.

## Proof plan

- Lowest test layer: scanner fixtures and correlation tests in `spacetop-core`, app worker/state tests for result application, Ratatui `TestBackend` tests for row and preview rendering.
- Required command: `cargo test` and `make lint`.
- Manual check, if any: optional local TUI check against `docs/spacetop-dev` with one running matched session.
- Docs/policy update needed: README or nearby docs only if the user-facing marker or configuration surface changes.
