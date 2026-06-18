---
title: Keep running state during Claude session activity
status: plan
source: "User report on 2026-06-18: while handling a Razorback workflow item in Claude Code, Spacetop marks it running from a session write and then clears it about every 2 seconds even though the agent is still active"
kind: bugfix
risk: medium
milestone: v1-maintenance
proof: cargo test -p spacetop-core session_activity and a focused TUI/app-state check for active write-event retention
started: 2026-06-18T09:40:52Z
completed:
verdict:
score: 0.88
worktree:
issue:
pr:
id: 071
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
