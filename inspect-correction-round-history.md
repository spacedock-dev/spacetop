---
id: 081
title: Inspect correction-round history
status: plan
source: "Captain-requested Spacedock release compatibility survey, 2026-10-01"
kind: feature
risk: medium
milestone: spacedock-0.27-compatibility
proof: "Round-pointer and JSONL fixtures, query ordering tests, app navigation tests, and TestBackend assertions; dropping earlier rounds or attributing a resolution to another Briefing must fail."
started: 2026-10-22T14:52:31Z
completed:
verdict:
score: 0.85
worktree:
issue:
pr:
mod-block:
---

Make recorded correction cycles inspectable without searching through entity folders. Spacedock retains review-round pointers plus immutable Briefing and review JSONL records.

## Scope

- Recommended build order: 3 of 5. Scores express the survey recommendation, not an approved schedule.
- Touches: parser / domain / query / app-state / UI / watcher / docs.
- Non-goals: Recording correction rounds, routing workers, changing acceptance criteria, or mutating immutable rooms.
- Coordinate with support-durable-gate-status for shared typed records; avoid duplicate parsing.

## Acceptance criteria

**AC-1:** Parse the current review-round pointer and discover retained review/<stage>/round-N records through a typed core boundary. Display stage, cycle, reviewer decision, and recorded follow-up dispositions where present.

**AC-2:** Allow users to browse rounds in deterministic order and distinguish the current pointer from older rounds. Show missing, malformed, mismatched, or incomplete evidence explicitly.

**AC-3:** Render recorded Annotation and Resolution entries with actor, time, rationale, and includes relationships; do not infer unrecorded responses, completion, or approval from prose.

**AC-4:** Cover multiple stages and cycles, legacy entities without review-round, malformed or partial JSONL, stale pointers, safe path boundaries, and refresh/rendering behavior.

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
