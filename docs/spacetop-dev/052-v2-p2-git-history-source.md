---
id: "052"
title: "v2 P2: git history source"
status: verify
source: "captain - reviewed SpaceTop v2 roadmap plan"
kind: feature
risk: high
milestone: v2-p2
proof: "cargo test --workspace; make lint; cargo test -p spacetop-core --test no_write_git_calls"
started: 2026-06-11
completed:
verdict:
score: 0.96
worktree: .worktrees/spacedock-ensign-052-v2-p2-git-history-source
issue:
pr:
mod-block: merge:pr-merge
---

Implement phase P2 of the SpaceTop v2 internals rebuild: derive trustworthy
per-entity stage history and metrics from read-only git history, including
frontmatter-only status changes, archive-move `done` synthesis, shallow-clone
refusal, and non-blocking TUI history ingestion.

Plan: `docs/superpowers/plans/2026-06-11-spacetop-v2-p2-git-history-source.md`

This entity was fired without dispatch at the captain's request.

## Scope

- Kind: feature
- Risk: high
- Milestone: v2-p2
- Touches: core git seam, git history source, metrics, index history state, TUI history worker, guardrail tests
- Non-goals: new P3 views, async runtime, workflow markdown writes, full git replay

## Acceptance criteria

**AC-1 -- History events are trustworthy or unavailable.**
Status events come only from entity frontmatter and archive renames; shallow clones, non-git repositories, and git errors return explicit unavailable reasons.
Verified by: real git fixture tests plus mocked runner tests.

**AC-2 -- Metrics are derived only from complete enough history.**
Dwell, cycle, WIP, and throughput metrics come from stored `StageEvent`s and are not computed from shallow or failed history.
Verified by: metrics tests and index history tests.

**AC-3 -- Git history remains read-only.**
History code uses only approved read commands and does not broaden the `git pull --ff-only` sync write path.
Verified by: behavioral runner assertions and `no_write_git_calls`.

**AC-4 -- TUI does not block on history ingestion.**
Overview load/reload shows active working-tree state immediately, marks history as loading, and folds worker results into the index later.
Verified by: app tests for pending and completed history worker results.

## Proof plan

- Lowest test layer: git fixture and metrics tests first, then app history-worker tests.
- Required commands: `cargo test --workspace`; `make lint`; `cargo test -p spacetop-core --test no_write_git_calls`.
- Manual check, if any: optional TUI smoke for loading/unavailable messages.
- Docs/policy update needed: document read-only history commands if behavior changes.

## Stage Report: implement

- DONE: Implemented trustworthy read-only git history in `spacetop-core`: shared `GitRunner` seam, first-parent frontmatter-only `StageEvent`s, explicit unavailable reasons, archive rename `done` synthesis, and stable timelines across renames.
- DONE: Wired history results into `WorkflowIndex` timeline, metrics, and activity APIs, plus TUI non-blocking history ingestion so active workflow state loads immediately while exact history availability is preserved.
- DONE: Proved the behavior with focused mocked-runner, real git fixture, metrics, index, app, and read-only guardrail tests. Verification passed: `cargo test --workspace`; `make lint`; `cargo test -p spacetop-core --test no_write_git_calls`.

### Summary

Implemented P2 git history source on branch `spacedock-ensign/052-v2-p2-git-history-source`. The core now derives per-entity stage timelines from read-only first-parent git history, refuses shallow/non-git history with explicit reasons, computes metrics only from stored events, and keeps TUI loading non-blocking through a background worker and guarded result application.
