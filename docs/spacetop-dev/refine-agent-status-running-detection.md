---
title: Refine agent status running detection
status: plan
source: User report on 2026-06-18 that Codex and Claude Code sessions never match PID checks
kind: bugfix
risk: medium
milestone: v1-maintenance
proof: Reproduce with real Codex and Claude Code session logs/process table, then prove the refined rule with core tests plus make lint
started: 2026-06-18T06:38:33Z
completed:
verdict:
score: 0.86
worktree:
issue:
pr:
id: 067
---

Codex and Claude Code session attribution currently treats running as a live PID match, but local experiments show real agent session logs often do not contain a reusable PID or do not match ps by PID. This makes the running state and active-session marker effectively unreachable.

Shape this task before implementation. Analyze the actual session artifacts and process-table signals for both Codex and Claude Code, refine the product semantics for running, recent, and stale, and then implement the smallest reliable rule.

Acceptance criteria:

- AC-1: The task documents the observed failure mode with real local evidence from both Codex and Claude Code session artifacts, without leaking transcript content.
- AC-2: The design states exactly what qualifies as running, recent, and stale, including confidence requirements for the list marker.
- AC-3: Running can be reached for currently active Codex and Claude Code sessions without depending only on a JSON pid field.
- AC-4: The implementation preserves stale/recent preview metadata and does not turn mtime-only evidence into a list marker.
- AC-5: Core tests cover the refined detection rule and the old PID-only failure pattern.
- AC-6: README or nearby docs are updated if the user-facing semantics change.
- AC-7: Verification includes cargo test and make lint, or records why a command could not run.
