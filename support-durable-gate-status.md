---
id: 079
title: Support durable gate status
status: plan
source: "Captain-requested Spacedock release compatibility survey, 2026-10-01"
kind: feature
risk: medium
milestone: spacedock-0.27-compatibility
proof: "Gate parsing, query/export, app state, and TestBackend fixtures; deleting gate parsing or conflating consumed approval with pending readiness must fail the checks."
started: 2026-10-01T10:07:39Z
completed:
verdict:
score: 0.95
worktree:
issue:
pr:
mod-block:
---

Show durable gate state even when the originating agent session is no longer available. Spacedock 0.27 stores gate attempts, resolutions, and approval application state in entity frontmatter; Spacetop currently discards these fields.

## Scope

- Recommended build order: 1 of 5. Scores express the survey recommendation, not an approved schedule.
- Touches: parser / domain / query / app-state / UI / export.
- Non-goals: Gate preparation, approval, consumption, merge, and all workflow-state writes.

## Acceptance criteria

**AC-1:** Parse versioned gates.records and their attempts, briefing bindings, withdrawals, resolutions, and application states into typed core data. Existing entities without gates remain readable; unsupported or invalid records produce diagnostics rather than approval claims.

**AC-2:** Show current-stage gate status in the task list and detail view, distinguishing awaiting captain, approved awaiting advance, and approved awaiting merge. Historical attempts must not be mistaken for current authority.

**AC-3:** Expose durable gate data through headless JSON export and query filters. Gate readiness remains separate from runtime activity; no session log is required to display a recorded gate.

**AC-4:** Cover pending, consumed, superseded, withdrawn, historical-stage, malformed, and legacy records with parser/query/app tests and TestBackend assertions. Use upstream readiness semantics as the compatibility reference, including preparation and terminal-merge boundaries.

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
