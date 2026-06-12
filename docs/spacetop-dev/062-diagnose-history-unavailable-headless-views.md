---
id: "062"
title: Diagnose history unavailable in metrics, activity, and timeline views
status: plan
source: captain diagnostic request 2026-06-12
kind: bugfix
risk: medium
milestone: v1-maintenance
proof: reproduced headless commands plus core/headless regression tests and make lint
started: 2026-06-12T13:01:29Z
completed:
verdict:
score: 0.86
worktree:
issue:
pr:
---

Metrics, activity, and timeline keep showing:

```text
history unavailable: git log could not be read
```

The failure reproduces in the current Spacetop project with
`docs/spacetop-dev`, even though the repository has readable git history for the
workflow path. The diagnostic pass should find the actual failing boundary and
fix it, or make the unavailable reason precise enough to act on if the history
source is legitimately incomplete.

## Reproduction evidence

Observed from the repo root on 2026-06-12:

```bash
cargo run -p spacetop -- metrics --workflow-dir docs/spacetop-dev
cargo run -p spacetop -- activity --workflow-dir docs/spacetop-dev
cargo run -p spacetop -- timeline 056 --workflow-dir docs/spacetop-dev
```

Each command completed successfully at the process level but printed:

```text
history unavailable: git log could not be read
```

Sanity checks from the same checkout:

```bash
git rev-parse --is-shallow-repository
git log --oneline -- docs/spacetop-dev
git log --first-parent --reverse --date=unix --pretty=format:%H%x00%ct --name-status -M -- 'docs/spacetop-dev/**'
```

The repository reported `false` for shallow status, and both git-log checks
returned workflow history. That suggests the user-facing message may be masking
a downstream history-loader error, such as `git show` or metadata extraction,
not necessarily the initial `git log` command.

## Scope

- Kind: bugfix
- Risk: medium
- Milestone: v1-maintenance
- Touches: git / index-query / headless CLI / UI history views
- Non-goals: changing Spacedock workflow markdown semantics, adding workflow
  write support, or filing a GitHub issue.

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 -- The root cause is identified with evidence.**
The implementation report names the exact failing operation and explains why the
current Spacetop project prints `history unavailable: git log could not be read`
despite readable workflow git history.
Verified by:

**AC-2 -- Headless metrics, activity, and timeline behave correctly for this repo.**
Running the reproduced commands against `docs/spacetop-dev` produces useful
history output when history is available, or a precise unavailable reason when it
is not.
Verified by:

**AC-3 -- TUI history views use the same corrected history result.**
The Metrics, Activity, and Timeline TUI pages stop showing the generic git-log
message for this reproducible case and stay consistent with headless output.
Verified by:

**AC-4 -- Regression coverage protects the history loader boundary.**
Tests cover the failing path at the lowest practical layer, including the case
where `git log` succeeds but later history processing fails.
Verified by:

**AC-5 -- Spacetop remains read-only toward workflow markdown.**
The fix may read git history and workflow files, but it does not add any path
that mutates workflow markdown.
Verified by:

## Proof plan

- Lowest test layer: `spacetop-core` git-history/index tests for the failing
  loader boundary, plus focused headless CLI tests for unavailable/error
  rendering.
- Required command: `make lint`
- Manual check, if any: run the three reproduction commands against
  `docs/spacetop-dev`, then open the TUI Metrics, Activity, and Timeline views.
- Docs/policy update needed: only if the user-facing unavailable messages or
  history-view behavior changes.
