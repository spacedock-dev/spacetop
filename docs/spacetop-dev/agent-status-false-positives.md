---
title: Agent status false positives on undispatched tasks
status: shape
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
