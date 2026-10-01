---
id: 080
title: Browse gate Briefings and decisions
status: plan
source: "Captain-requested Spacedock release compatibility survey, 2026-10-01"
kind: feature
risk: medium
milestone: spacedock-0.27-compatibility
proof: "Briefing parser/path-boundary fixtures, app navigation tests, watcher refresh checks, and TestBackend assertions; wrong bindings and escaped refs must be rejected."
started: 2026-10-01T18:17:27Z
completed:
verdict:
score: 0.90
worktree:
issue:
pr:
mod-block:
---

Let users inspect a gate question, its evidence, and its recorded decision from Spacetop. Spacedock 0.27.1 prepares one canonical Briefing instead of requiring request.json.

## Scope

- Recommended build order: 2 of 5. Scores express the survey recommendation, not an approved schedule.
- Touches: parser / domain / app-state / UI / watcher / docs.
- Non-goals: Approval controls, embedded external providers, remote artifact fetching, or editing review evidence.
- Coordinate with support-durable-gate-status for shared typed records; avoid duplicate parsing.

## Acceptance criteria

**AC-1:** Resolve the selected attempt's briefing.room-ref and display the canonical Briefing question and artifact/reference links. Support current index.json, retained briefing.json, and documented legacy bindings without requiring request.json for current rooms.

**AC-2:** Display recorded decision, actor, timestamp, reason, withdrawal, and application state with clear attribution. Missing files, invalid identities, or digest mismatches show diagnostics rather than verified evidence.

**AC-3:** Keep referenced-file reads within explicitly supported safe roots; reject traversal and symlink escapes. The viewer never records, approves, consumes, or repairs a gate.

**AC-4:** Provide predictable keyboard navigation and narrow-terminal rendering. Prove room-format handling, safe path resolution, refresh behavior, and rendered details at the lowest practical test layer.

## Proof plan

- Use parser/scanner and query tests for core facts, app tests for state/input, and Ratatui TestBackend for rendering.
- Run cargo fmt, cargo test, and make lint for code changes. Run the ignored notify smoke test when watcher backend behavior changes.
- Update README and nearby docs for changed user-visible behavior.
- Preserve the read-only workflow contract and existing git/config/session guardrails.

## Survey sources

- Stable release: https://github.com/spacedock-dev/spacedock/releases/tag/v0.27.3
- Durable gate release: https://github.com/spacedock-dev/spacedock/releases/tag/v0.27.0
- Canonical room change: https://github.com/spacedock-dev/spacedock/releases/tag/v0.27.1
- Edge dispatch change: https://github.com/spacedock-dev/spacedock/releases/tag/v0.28.0-pre3
- Spacetop survey baseline: e503807 (main, 2026-08-24).

## Survey evidence

Current Spacetop successfully exported a temporary folder-form entity copied from the Spacedock 0.27.3 advisory-round fixture, but gates and review-round were absent from its typed export. This confirms metadata tolerance and missing projection, not full runtime compatibility. The survey made no production code changes.
