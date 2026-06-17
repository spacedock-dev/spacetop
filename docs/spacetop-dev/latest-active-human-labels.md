---
title: Human-readable active-session state and latest time
status: verify
source: User feedback, 2026-06-17
kind: bugfix
risk: low
milestone: v1-maintenance
proof: Ratatui preview test plus make lint
started: 2026-06-17T06:36:07Z
completed:
verdict:
score: 0.72
worktree: .worktrees/spacedock-ensign-latest-active-human-labels
issue:
pr:
id: 066
mod-block: merge:pr-merge
---

Fix two UI readability issues in the active-session metadata shown for a selected task.

## Problem

- The three internal states are acceptable, but the displayed recent/stale wording is not self-describing.
- The latest activity value is currently a raw timestamp, which is not suitable for humans reading the TUI.

## Acceptance criteria

- **AC-1** The preview keeps the same three-state model but renders recent/stale with self-describing human-facing labels.
- **AC-2** The preview renders latest active time in a human-readable form instead of raw Unix seconds.
- **AC-3** A focused rendering test covers the changed output.

## Stage Report: plan

- DONE: Plan names the exact minimal files/modules to change for human-readable state labels and latest-active time.
  Change `crates/spacetop/src/ui/preview.rs` only: keep `AgentSessionState` unchanged, add tiny UI-local formatting helpers used by `session_attribution_line`, render `Recent` as `recent activity` and `Stale` as `stale activity`, leave `Running` as `running`, and format `latest_activity_unix` with stdlib `std::time::{UNIX_EPOCH, Duration}` plus existing Rust formatting.
- DONE: Plan identifies the lowest practical test covering the preview output.
  Update `crates/spacetop/src/ui/tests/preview.rs::preview_renders_session_metadata_without_transcript_content` to assert the new label text and a human-readable latest value instead of `latest: 1718000000`; if needed, extend the existing `app_with_active_session_marker` helper in `crates/spacetop/src/ui/tests.rs` just enough to exercise `Recent`/`Stale`.
- DONE: Plan preserves the read-first contract and avoids new dependencies.
  No parser, app-state, workflow markdown writer, git sync, config/session, or dependency changes; proof remains `cargo test -p spacetop ui::tests::preview_renders_session_metadata_without_transcript_content -- --exact`, `cargo test`, and `make lint`.

### Summary

Use the existing preview rendering path and its existing Ratatui regression test. Keep the fix UI-local: the domain model still exposes the three internal states and Unix activity seconds, while the preview converts only the displayed words and timestamp into human-readable text.

## Stage Report: implement

- DONE: Preview renders recent/stale with self-describing human-facing labels while preserving the internal three-state model.
  Commit 9b07b4e keeps `AgentSessionState` unchanged and maps only preview display text to `recent activity` and `stale activity`.
- DONE: Preview renders latest activity as a human-readable latest-active value instead of raw Unix seconds.
  Commit 9b07b4e formats `latest_activity_unix` as a UTC calendar timestamp in the preview.
- DONE: Focused preview test is updated and required Rust checks are run or clearly reported if blocked.
  Ran `cargo test -p spacetop ui::tests::preview::preview_renders_session_metadata_without_transcript_content -- --exact`, `cargo test`, and `make lint`; all passed.

### Summary

Implemented the smallest UI-local formatting change for active-session preview metadata. The focused Ratatui preview test now covers the expanded recent/stale labels and verifies the latest activity value no longer renders as raw Unix seconds.
