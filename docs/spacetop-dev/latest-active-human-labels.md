---
title: Human-readable active-session state and latest time
status: plan
source: User feedback, 2026-06-17
kind: bugfix
risk: low
milestone: v1-maintenance
proof: Ratatui preview test plus make lint
started: 2026-06-17T06:36:07Z
completed:
verdict:
score: 0.72
worktree:
issue:
pr:
id: 066
---

Fix two UI readability issues in the active-session metadata shown for a selected task.

## Problem

- The three internal states are acceptable, but the displayed recent/stale wording is not self-describing.
- The latest activity value is currently a raw timestamp, which is not suitable for humans reading the TUI.

## Acceptance criteria

- **AC-1** The preview keeps the same three-state model but renders recent/stale with self-describing human-facing labels.
- **AC-2** The preview renders latest active time in a human-readable form instead of raw Unix seconds.
- **AC-3** A focused rendering test covers the changed output.
