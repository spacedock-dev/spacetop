---
title: Keep running state during Claude session activity
status: done
source: "User report on 2026-06-18: while handling a Razorback workflow item in Claude Code, Spacetop marks it running from a session write and then clears it about every 2 seconds even though the agent is still active"
kind: bugfix
risk: medium
milestone: v1-maintenance
proof: cargo test -p spacetop-core session_activity and a focused TUI/app-state check for active write-event retention
started: 2026-06-18T09:40:52Z
completed: 2026-06-18T10:26:51Z
verdict: PASSED
score: 0.88
worktree:
issue:
pr: "#72"
id: 071
mod-block:
archived: 2026-06-18T10:26:55Z
---

Spacetop should keep an item in the running state while the matching Claude Code or Codex session continues to show credible live activity. The current behavior can mark a matched session as running from a write event, then clear that running state on the short cleanup cadence, which makes an active worker appear to flicker between running and recent.

Observed case: the Razorback workflow item `spider2-dbt-harbor-view-ade-parity` was being handled by Claude Code. The preview showed `agent: Claude Code`, `session: subagents high`, `state: running`, `via: write`, and `latest: just now`, but the running state was cleaned up roughly every 2 seconds while the Claude Code work was still ongoing.

## Scope

- Kind: bugfix
- Risk: medium
- Milestone: v1-maintenance
- Touches: session activity / app-state / watcher / UI
- Non-goals: replace PID or resume-UUID signals entirely, add workflow writes from Spacetop, or infer task ownership from broad ID-only mentions.

## Acceptance criteria

**AC-1 -- Live write-event activity is retained long enough to represent an active agent session.**
Verified by: focused session activity test covering a matched Claude/Codex session that receives write events and remains running across the cleanup cadence.

**AC-2 -- Running state does not flicker back to recent while the same matching session continues to update.**
Verified by: app-state or integration-level test that exercises repeated refresh/cleanup ticks with a recent matching update.

**AC-3 -- Stale sessions still decay out of running state after the intended grace period.**
Verified by: test coverage for no further writes after the grace window, proving the state changes to recent or stale instead of staying running forever.

**AC-4 -- Ownership matching remains precise.**
Verified by: regression coverage showing unrelated workflow/session mentions do not mark a task running.

## Proof plan

- Lowest test layer: `spacetop-core` session activity tests, plus app-state tests only if cleanup retention lives outside core.
- Required command: `cargo test -p spacetop-core session_activity` and `make lint`.
- Manual check, if any: run `cargo run -p spacetop -- --workflow-dir docs/spacetop-dev` while an active Claude/Codex session updates a matched workflow item, and confirm the row does not flicker every cleanup tick.
- Docs/policy update needed: update nearby session-status docs if the grace period or state semantics change.

## Stage Report

### Summary

Planned the task 071 fix path without code changes. The likely drop path is the 2s periodic session scan replacing the current attribution set after write-derived running evidence is lost or omitted. The implementation should stay in `spacetop-core::session_activity` unless an app-level regression proves the replacement semantics also need a narrow guard.

### Checklist

- DONE: Identified the cleanup/expiry path: `SessionActivityWorkerState::request_periodic_scan` runs every 2s, passes `previous_session_files`, core derives `ObservedSessionWrite` from `observed_running_until_unix`, and `WorkflowIndex::replace_session_scan_report` replaces all attributions on each successful scan.
- DONE: Named minimal owned modules: `crates/spacetop-core/src/session_activity.rs` first, with `crates/spacetop/src/app/overview.rs` and `crates/spacetop-core/src/index.rs` only if app-level replacement needs a retention guard.
- DONE: Planned lowest-layer tests for Claude/Codex write retention across periodic scans, stale decay after the grace window, deadline extension on repeated writes, and unrelated-session non-attribution.
- DONE: Preserved precise session ownership matching; the plan explicitly keeps ID-only and conflicting-dispatch evidence from marking an entity running.
- SKIPPED: Code implementation, because this was the `plan` stage only.
- SKIPPED: `make lint`, because no Rust code was changed in this stage.
- FAILED: None.

### Verification

- `cargo test -p spacetop-core session_activity -- --nocapture` passed: 21 tests passed, 0 failed.

## Stage Report

### Summary

Implemented task 071 in `crates/spacetop-core/src/session_activity.rs` with a core-only fix. Session activity scans now retain readable session-file snapshots before entity matching, so write-derived liveness from a matching Claude Code or Codex session survives periodic cleanup scans until the observed write grace window expires. Ownership matching stayed unchanged and precise.

Committed implementation as `7f9c74e Retain session write snapshots across scans`.

### Checklist

- DONE: Added failing-then-passing core regression coverage for a previously unmatched session file that later receives matching write activity and remains `running` across periodic scans.
- DONE: Covered stale decay after the observed write grace window; the regression proves the state returns to `recent` after no further writes.
- DONE: Kept the fix in `spacetop-core::session_activity` by retaining scanned readable session-file snapshots before entity matching instead of moving retention rules into UI or app state.
- DONE: Preserved precise ownership matching; unrelated/id-only and conflicting-dispatch session cases remain covered by existing regression tests.
- DONE: Updated the unrelated-session guard to assert the intended invariant: unrelated sessions do not produce attribution or trigger resume-command probing.
- SKIPPED: App-level test changes, because the fix landed entirely in core and the worker already feeds core snapshots back into later scans.
- SKIPPED: Documentation changes outside this task report, because no user-facing command, label, or semantic docs changed.
- FAILED: None.

### Verification

- Red test observed: `cargo test -p spacetop-core previously_unmatched_session_write_marks_running_until_grace_window_expires -- --nocapture` failed before the fix with `left: Recent` and `right: Running`.
- `cargo test -p spacetop-core session_activity -- --nocapture` passed after the fix: 22 tests passed, 0 failed.
- `cargo fmt` ran after the Rust change.
- `make lint` passed; it ran `cargo clippy --all-targets --all-features -- -D warnings`.
- `git diff --check` passed.

## Stage Report: verify

- DONE: Independently verify the write-derived running retention fix and stale-decay behavior from the implementation branch.
  PASS: `previously_unmatched_session_write_marks_running_until_grace_window_expires` covers write-derived running retention until `OBSERVED_RUNNING_WINDOW` and decay back to `Recent` after the grace window.
- DONE: Confirm ownership matching remains precise and unrelated/id-only sessions are not attributed.
  PASS: `session_activity` regressions cover id-only evidence, unrelated workspace sessions, incidental same-repo ID mentions, conflicting dispatch assignments, and folder `index.md` false matches.
- DONE: Check every acceptance criterion has code/test evidence and report PASS or REJECT with commands run.
  PASS: AC-1 through AC-4 are covered in `crates/spacetop-core/src/session_activity.rs`; no app-level change is needed because snapshot retention is produced by core and consumed by the existing scan replacement path.
- SKIPPED: Manual TUI check.
  The acceptance criteria are covered at the focused core layer and `make lint` passed; no terminal-only behavior changed.
- FAILED: None.

### Summary

PASS. The branch retains readable session-file snapshots before entity matching, which lets a later matching Claude Code or Codex write become `ObservedSessionWrite` and stay `running` across cleanup ticks until the intended grace window expires. I found no review findings, and verification passed with `cargo test -p spacetop-core session_activity -- --nocapture`, `git diff --check main...HEAD`, and `make lint`.
