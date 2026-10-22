---
title: Show local refresh and Git sync freshness
status: shape
source: "User-requested product improvement brief, 2026-10-22"
kind: feature
risk: medium
milestone: usability
proof: "Behavior scenarios and a live terminal walkthrough; see acceptance criteria and proof plan."
started:
completed:
verdict:
score:
worktree:
issue:
pr:
mod-block:
id: 089
---

Help users understand how current the displayed information is without confusing local file refresh with shared Git updates.

## Scope

- Kind: feature
- Risk: medium
- Milestone: usability
- Touches: app-state / UI / git / watcher / docs
- Show the last successful local read separately from the last successful Git sync observed by Spacetop.
- Explain storage conditions that prevent sync, including partial definition-only sync.
- Non-goals: Background network polling, automatic sync, checkout repair, new Git write commands, or claiming the remote has no newer changes.

## Acceptance criteria

**AC-1 -- Local reads and Git sync have separate meanings.**
Users can identify when Spacetop last successfully read local files and when it last observed a successful Git sync. Unknown or never-observed times are explicit rather than invented.
Verified by: App-state scenarios for startup, local refresh, successful sync, and unknown sync history; a terminal walkthrough explaining both indicators.

**AC-2 -- Failure never advances a success timestamp.**
Failed reload or sync operations preserve the relevant prior success time and expose the failure. A local file change cannot claim that a Git sync occurred.
Verified by: Deterministic success-to-failure transitions and local-change-only scenarios.

**AC-3 -- Partial sync cannot look like full freshness.**
For split-root workflows, users can distinguish definition sync from state sync. Missing, detached, wrong-branch, or unverified state produces a clear reason when state sync is skipped.
Verified by: App and Git-sync scenarios for fully successful, partially successful, skipped, and failed sync outcomes.

**AC-4 -- Visibility does not broaden writes.**
No new background sync or checkout repair occurs. Explicit sync remains limited to the existing audited fast-forward path and eligible roots.
Verified by: Existing Git write guardrails and a walkthrough without any sync action, followed by an explicit sync in a disposable fixture.

## Proof plan

- Lowest test layer: Typed domain/query and app-state behavior, with rendering checks where presentation affects interpretation.
- Required commands for implementation: cargo fmt, cargo test, and make lint. Use existing safety checks for the affected surfaces.
- Manual check: Run the actual terminal UI with representative normal, missing-data, and failure cases. Record evidence for each acceptance criterion.
- Docs update: Update README and relevant user guidance for the delivered behavior. Keep workflow files read-only during product use.
