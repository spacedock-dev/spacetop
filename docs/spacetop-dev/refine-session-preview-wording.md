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
