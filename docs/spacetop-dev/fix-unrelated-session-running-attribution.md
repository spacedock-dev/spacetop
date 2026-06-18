---
title: Fix unrelated session running attribution
status: verify
source: "Follow-up from 2026-06-18 screenshot: newly created task 068 is shown running from an unrelated dataagentbench Codex session"
kind: bug
risk: medium
milestone: v1-maintenance
proof: Focused session-activity tests with unrelated workspace/session fixtures plus a preview/task-list rendering regression
score: 0.86
id: 070
started: 2026-06-18T08:27:34Z
worktree: .worktrees/spacedock-ensign-fix-unrelated-session-running-attribution
mod-block: merge:pr-merge
---

# Fix unrelated session running attribution

Spacetop can still mark a newly created Spacedock task as `running` by attaching an unrelated agent session from another workspace. The observed false positive is task `068` in `docs/spacetop-dev`, where the preview shows a Codex session rooted under `dataagentbench` and reports `state: running` even though that session has no relationship to the Spacetop workflow task.

## Acceptance criteria

- **AC-1:** Running/recent/stale/human-gated evidence must be scoped to a session that is actually related to the workflow task, not just a loose numeric or text match.
- **AC-2:** A Codex or Claude session rooted in an unrelated workspace, such as `dataagentbench`, must not mark a Spacetop task as `running` merely because the transcript contains broad matching text.
- **AC-3:** The session attribution path should make the accepted relationship explicit in code: task id, slug, workflow path, worktree path, or other deliberate evidence.
- **AC-4:** Tests must cover the screenshot-shaped false positive: a new task with an unrelated active session should render as non-running.
- **AC-5:** Verification includes focused core tests for session attribution and the relevant TUI rendering test, plus `make lint`.

## Stage Report: plan

- DONE: Map the current false-positive path from session artifact matching through displayed running state.
  `match_entity()` currently accepts task-id/slug-only low-confidence matches; `RunStateClassifier` can then upgrade that unrelated matched session to `running`, and the UI renders the resulting attribution.
- DONE: Define the smallest accepted relationship rule and exact owned files/tests for implementation.
  Reject id-only/slug-only matches; keep high worktree matches and medium workflow/repo-plus-id-or-slug matches in `crates/spacetop-core/src/session_activity.rs`, with focused core plus preview/task-list rendering regressions.
- DONE: Specify verification commands for the screenshot-shaped unrelated workspace regression.
  Run `cargo test -p spacetop-core session_activity`, focused `spacetop` preview/task-list tests, full `cargo test`, and `make lint`.

### Summary

The plan keeps the fix in the core session-attribution matcher instead of adding UI state or new abstractions. The intended implementation is to remove loose id/slug-only attribution so unrelated live sessions, including sessions rooted under `dataagentbench`, cannot produce a Spacetop task `running` marker.

## Stage Report: implement

- DONE: Reject id-only and slug-only session matches in `crates/spacetop-core/src/session_activity.rs`.
  `match_entity()` now returns medium confidence only when workflow/repo evidence is present together with the entity id or slug; worktree and worktree-source matches remain high confidence.
- DONE: Added screenshot-shaped regressions for the unrelated live `dataagentbench` session case.
  Core coverage proves a live Codex session rooted elsewhere that mentions task `068` produces no attribution, and the preview rendering regression proves the app renders no running marker or session metadata for that scan result.
- DONE: Ran focused and required verification.
  `cargo test -p spacetop-core session_activity`, `cargo test -p spacetop preview_omits_session_metadata_for_unrelated_running_session`, `cargo test -p spacetop task_row_renders_active_session_marker_from_typed_attribution`, `cargo test`, and `make lint` passed. The existing real-notify watcher tests remained ignored under the default `cargo test` run.

### Summary

Implemented the matcher fix by removing loose low-confidence id/slug-only attribution. Unrelated live sessions can no longer mark a Spacetop task as `running` unless the transcript also contains deliberate workflow/repo or worktree relationship evidence.
