---
id: 078
title: Stop runtime activity indicator flickering between scans
status: plan
source: "Captain report: Engram runtime indicator alternates on and off every few seconds while Spacetop is running"
kind: bug
risk: high
milestone: v1-maintenance
proof: Deterministic consecutive-scan regression plus a live Engram workflow check
started: 2026-08-21T14:53:21Z
completed:
verdict:
score: 0.86
worktree:
issue:
pr:
mod-block:
---

Spacetop intermittently alternates an entity runtime marker between running and idle every few seconds while the associated agent session remains active. The symptom is visible against the Engram workflow at `/Users/kent/Dev/InfuseAI/Spacedock/engram/.spacedock/reflection-dev` and matches Spacetop periodic session-activity refresh cadence. Reproduce the transition and make runtime state remain truthful and stable until correlated structured evidence changes it.

## Scope

- Kind: bug investigation and correctness fix
- Risk: high because a flickering runtime marker gives operators contradictory live state and can hide an active worker
- Milestone: v1-maintenance
- Touches: session activity incremental scan state, Codex and Claude event correlation, reducer state, periodic worker result application, entity attribution, UI marker regressions, and nearby documentation if behavior changes
- Non-goals: mutating the Engram workflow; inferring activity from process presence, mtimes, generic path mentions, or filesystem writes; increasing the polling interval merely to hide the flicker; redesigning the activity UI

## Acceptance criteria

**AC-1 -- The reported running-to-idle-to-running sequence is reproduced with no real lifecycle boundary.** Consecutive scans of one active Engram-derived or equivalent sanitized session show the exact false transition and identify which retained evidence, cursor, attribution, or reducer input changes between scans.
Verified by: a deterministic regression fixture that fails before the fix and records at least three consecutive scan outcomes over unchanged active-session evidence.

**AC-2 -- A correlated active session remains visibly running across periodic refreshes.** After canonical dispatch/session correlation and a structured start establish running, unchanged and append-only scans preserve the same running attribution until a correlated structured stop, gate, or superseding lifecycle event changes it.
Verified by: lowest-layer session activity tests covering initial, unchanged, append-only, and subsequent periodic scan state, plus an app-level result-application assertion.

**AC-3 -- The fix preserves strict evidence boundaries.** Uncorrelated sessions, process presence, mtimes, generic path mentions, and filesystem writes alone remain idle; scan errors preserve the last good snapshot; truncation, rotation, and deletion do not manufacture running state.
Verified by: focused negative and lifecycle regressions alongside the existing cursor and scan-failure suites.

**AC-4 -- The Engram scenario no longer flickers.** With Spacetop open on the Engram reflection workflow while its attributed agent session remains active, the runtime marker stays continuously on across multiple two-second polling cycles and turns off only after an actual structured stop or gate transition.
Verified by: a live observation log or repeatable probe covering at least five consecutive polling cycles, with timestamps and the correlated session/entity IDs sanitized in durable evidence.

**AC-5 -- Existing runtime detection and repository quality gates remain green.** Codex and Claude correlation, worker reuse, gate display, incremental scanning, UI rendering, read-only guardrails, and full workspace behavior do not regress.
Verified by: focused session-activity and app/UI tests, full `cargo test`, `cargo fmt --all -- --check`, `make lint`, and `git diff --check`.

## Proof plan

- Lowest test layer: session activity state/reducer and source-specific correlation tests, then app worker result application and Ratatui marker coverage only where needed.
- Required command: focused `cargo test` filters first, then `cargo test`, `cargo fmt --all -- --check`, `make lint`, and `git diff --check`.
- Manual check, if any: run Spacetop against the Engram reflection workflow during an attributed live agent session and observe at least five polling cycles.
- Docs/policy update needed: update nearby runtime-activity documentation only if the corrected lifecycle contract is not already stated accurately.
