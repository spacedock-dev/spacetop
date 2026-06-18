---
title: Refine session preview wording
status: verify
source: "Follow-up from task 067 on 2026-06-18: preview metadata wording is too implementation-heavy"
kind: feature
risk: low
milestone: v1-maintenance
proof: Ratatui preview rendering tests covering the revised session metadata line
started: 2026-06-18T10:28:19Z
completed:
verdict:
score: 0.78
worktree: .worktrees/spacedock-ensign-refine-session-preview-wording
issue:
pr: "#73"
id: 068
mod-block: merge:pr-merge
---

Refine the selected-task preview metadata for agent session attribution. The current wording exposes implementation details such as `session: xxx <effort>` and `via: <reason>` in the preview header, which is useful for debugging but too noisy for ordinary inspection.

Acceptance criteria:

- AC-1: The preview no longer renders `via: <reason>` in the normal selected-task header.
- AC-2: The session label wording is clearer than `session: xxx <effort>` while still preserving useful agent/session confidence information somewhere appropriate.
- AC-3: Task-list active marker behavior remains unchanged.
- AC-4: Ratatui preview tests cover the revised wording and ensure the removed `via:` label does not reappear.
- AC-5: Verification includes `cargo test -p spacetop` for relevant UI tests and `make lint`, or records blockers.

## Stage Report

Stage: plan

Checklist:

- DONE: Identified the selected-task preview metadata rendering path at `crates/spacetop/src/ui/preview.rs`, specifically `session_attribution_line`.
- DONE: Chose the minimal wording change: render `session: Mendel` and a separate `confidence: high`, while removing `via: <reason>` from the normal preview header.
- DONE: Kept the plan scoped away from running-state detection, session scan logic, and task-list active marker behavior.
- DONE: Named focused UI coverage in `crates/spacetop/src/ui/tests/preview.rs` for revised preview wording and `crates/spacetop/src/ui/tests/task_list.rs` for unchanged active markers.
- SKIPPED: Code implementation, because this stage was plan-only.
- SKIPPED: Test execution, because no code was changed in the plan stage.
- FAILED: None.

Plan summary:

Update only the preview header assembly in `session_attribution_line` so the selected-task preview shows:

`agent: Codex  ·  session: Mendel  ·  confidence: high  ·  state: recent  ·  latest: 3h ago`

The implementation should remove the `via:` segment entirely from the normal preview header, keep `state:` terse (`running`, `recent`, `stale`), and leave core attribution/liveness semantics untouched.

Verification commands for the implementation stage:

```bash
cargo test -p spacetop preview_renders_session_metadata_without_transcript_content
cargo test -p spacetop task_row_renders_active_session_marker_from_typed_attribution
cargo test -p spacetop
make lint
```

## Stage Report: implement

- DONE: Update the selected-task preview session metadata wording to remove `via:` and split confidence from the session label.
  Implemented in commit `ef227f8`; preview now renders `session: Mendel` and `confidence: high` as separate segments.
- DONE: Add or update focused Ratatui preview tests that fail if `via:` returns or confidence is folded into `session:`.
  `cargo test -p spacetop preview_renders_session_metadata_without_transcript_content` passed and pins both regressions.
- DONE: Preserve task-list active marker behavior and avoid changing session attribution/running-state logic.
  `cargo test -p spacetop task_row_renders_active_session_marker_from_typed_attribution` passed; no core session files or task-list rendering were edited.
- SKIPPED: None.
- FAILED: None.

### Summary

Refined the selected-task preview header wording so normal attribution metadata no longer exposes the liveness `via:` reason and confidence is no longer folded into the session label. Verification passed with the focused preview test, task-list guard test, full `cargo test -p spacetop`, `cargo fmt`, and `make lint`.

## Stage Report: verify

- DONE: Verify the preview header no longer renders `via:` and separates session confidence from the session label.
  PASS: Diff changes only `session_attribution_line` preview segments; focused preview test passed and asserts `session: Mendel`, `confidence: high`, no `session: Mendel high`, and no `via:`.
- DONE: Confirm task-list active marker behavior and session attribution logic were not changed.
  PASS: Diff touches no task-list or core session attribution files, and `cargo test -p spacetop task_row_renders_active_session_marker_from_typed_attribution` passed.
- DONE: Check every acceptance criterion has test evidence and report PASS or REJECT with commands run.
  PASS: AC-1 through AC-5 are covered by focused preview assertions, the task-list guard, `cargo test -p spacetop`, and `make lint`; verdict is PASS.
- SKIPPED: None.
- FAILED: None.

### Summary

Verified task 068 against branch `spacedock-ensign/refine-session-preview-wording`. The implementation satisfies the acceptance criteria: normal preview metadata omits `via:`, splits confidence into its own segment, preserves terse session state wording, and leaves task-list active marker behavior unchanged.

Commands run: `cargo test -p spacetop preview_renders_session_metadata_without_transcript_content`; `cargo test -p spacetop task_row_renders_active_session_marker_from_typed_attribution`; `cargo test -p spacetop`; `make lint`.
