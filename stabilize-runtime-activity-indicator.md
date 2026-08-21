---
id: 078
title: Stop runtime activity indicator flickering between scans
status: verify
source: "Captain report: Engram runtime indicator alternates on and off every few seconds while Spacetop is running"
kind: bug
risk: high
milestone: v1-maintenance
proof: Deterministic consecutive-scan regression plus a live Engram workflow check
started: 2026-08-21T14:53:21Z
completed:
verdict:
score: 0.86
worktree: .worktrees/spacedock-ensign-stabilize-runtime-activity-indicator
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

## Implementation plan

1. Pin the visible transition at the application boundary before changing production code.
   Extend the sanitized Codex scanner replay in `crates/spacetop/src/ui/tests/task_list.rs` and add an app regression in `crates/spacetop/src/app/tests.rs`: apply a running report, reload the unchanged entity snapshot, apply the next unchanged scan, and record the pre-fix visible sequence as `running -> idle -> running` even though both scan reports remain running.
2. Preserve the last successful activity snapshot across workflow-data reloads in the typed index layer.
   Add a narrow `WorkflowIndex` transfer method in `crates/spacetop-core/src/index.rs`; before `OverviewState::reload_from_index` replaces its index, copy activity and scanner-error state only for active entities whose ID and path match the prior index, then remove the unconditional `clear_entity_activities` call in `crates/spacetop/src/app/overview.rs`.
3. Keep attribution fail-closed when workflow identity actually changes.
   Core index tests must prove same-ID/same-path entities retain their typed `Running` or `HumanGate` state, while removed entities, new entities, renamed paths, and reused IDs do not inherit an old session; a fresh structured scan remains the only way to attribute those entities.
4. Exercise the full lifecycle without changing scanner semantics.
   The UI replay must render running before and after a workflow reload and an unchanged append-only scan, then clear only after the existing exact `task_complete` fixture; existing Codex/Claude correlation, reducer ordering, truncation/rotation/deletion, unlinked-session, wrong-parent/path/cwd, text-only, and scan-failure tests remain mandatory.
5. Keep documentation aligned with the corrected ownership boundary.
   Update `docs/superpowers/specs/2026-07-27-spacetop-entity-activity-design.md` to state that a successful workflow snapshot reload preserves the last good activity only for the same active entity identity; workflow reloads do not synthesize lifecycle events.
6. Prove the Engram behavior and repository gates.
   During an attributed Engram reflection session, observe the marker for at least five consecutive two-second cycles and record UTC timestamps plus sanitized entity/session IDs in the verify-stage report; pair that observation with the structured terminal regression, then run the focused session/app/UI tests, `cargo test`, `cargo fmt --all -- --check`, `make lint`, and `git diff --check`.

### Owned modules and focused commands

- Core ownership: `crates/spacetop-core/src/index.rs`; no planned changes to `session_activity/{state,codex,claude,reducer}.rs` because retained evidence already stays running across unchanged scans.
- App ownership: `crates/spacetop/src/app/overview.rs` and `crates/spacetop/src/app/tests.rs`; the worker in `app/session_activity_worker.rs` and two-second scheduler in `lib.rs` stay unchanged.
- Render proof: `crates/spacetop/src/ui/tests/task_list.rs`; production UI rendering stays unchanged.
- Focused proof: `cargo test -p spacetop-core session_activity`, `cargo test -p spacetop-core session_scan_report_indexes_activity_by_entity_id`, `cargo test -p spacetop session_activity`, and `cargo test -p spacetop task_row_renders_scanner_replay_then_clears_on_terminal_report`.

## Stage Report: plan

- DONE: Identify the exact incremental-scan, attribution, reducer, or result-application path that can reproduce running to idle to running without a lifecycle boundary.
  `OverviewState::reload_from_index` replaces `WorkflowIndex`, then clears activity; watcher reloads therefore erase a good running report until the next two-second scan reapplies the unchanged retained evidence.
- DONE: Produce an implementation plan naming owned modules and the lowest-layer regressions while preserving strict structured-evidence boundaries.
  The plan confines production changes to typed index-state transfer plus app reload wiring, matches ID and path, and keeps all source correlation and reducer rules unchanged.
- DONE: Define proof for five stable Engram polling cycles plus focused tests and the full required repository gates.
  The proof records five timestamped Engram observations, exercises a structured stop, runs focused core/app/UI regressions, then runs test, format, lint, and diff gates.

### Summary

The scanner already retains correlated lifecycle evidence across unchanged, append-only, truncated, rotated, and deleted logs; the false idle is introduced later when a workflow reload discards the published activity map. The implementation should carry the last good typed attribution across reload only for the same active entity identity, prove fail-closed behavior for identity changes, and leave polling and evidence rules intact.

## Stage Report: implement

- DONE: Reproduce and fix loss of the last good typed runtime attribution across workflow reloads, retaining it only when active entity ID and path still match.
  Commit `96cfde1` transfers the last published activity at the typed index boundary; the new core regressions fail if same-identity state is lost or changed identity inherits it.
- DONE: Add lowest-layer core/app/UI regressions proving running survives reload and periodic scans, clears on structured lifecycle evidence, and never leaks across changed identity.
  Core tests pin `Running`, `HumanGate`, scanner diagnostics, and fail-closed identity changes; the app test pins reload/result application; the UI replay fails if any of five reload/poll cycles flickers or exact `task_complete` does not clear.
- SKIPPED: Prove at least five stable Engram polling cycles, update the nearby activity contract if needed, and run all focused plus required repository gates.
  At `2026-08-21T15:11:53Z` the exact Engram workflow rendered zero active items because both reflection changes are archived, so no live marker existed to observe without forbidden workflow mutation; the deterministic sanitized replay proved five cycles instead, the contract was updated, and every gate below passed.
- DONE: Keep the nearby activity contract aligned with the corrected reload ownership boundary.
  The entity-activity design now states that reload preserves only same-ID/same-path active attribution and does not create a lifecycle event.
- DONE: Run all focused and required repository gates.
  `cargo test -p spacetop-core session_activity`, the report-index filter, app activity filters, the five-cycle UI replay, full `cargo test`, `cargo fmt --all -- --check`, `make lint`, and `git diff --check` all passed; each fails on the behavior or repository rule it names.

### Summary

Workflow reloads now retain the last good typed runtime attribution for unchanged active identity instead of briefly replacing it with idle. The evidence scanner and polling cadence are unchanged, identity changes still fail closed, and an exact structured stop still clears the marker; live Engram observation remains for a verify run when that workflow next has an active attributed entity.
