---
id: 064
title: Centralize entity identity module
status: plan
source: Architecture refactor scan on 2026-06-17; implement using Ponytail mode
kind: refactor
risk: medium
milestone: v2-later
proof: cargo test -p spacetop-core entity_identity && cargo test -p spacetop app::tests && make lint
started: 2026-06-17T02:33:11Z
completed:
verdict:
score: 0.86
worktree:
issue:
pr:
---

Create a focused refactor that centralizes workflow entity identity rules so parser, worktree merge, index lookup, and TUI selection stop re-implementing slug derivation independently.

Implementation must use Ponytail mode. If the Ponytail plugin/tooling is unavailable in the implementing session, the worker must stop and report that blocker instead of silently proceeding in another mode.

## Scope

- Kind: refactor
- Risk: medium
- Milestone: v2-later
- Touches: parser / app-state / core index
- Non-goals: do not split the Cargo workspace; do not split ui/graph.rs; do not refactor watcher.rs; do not broaden workflow write behavior.

## Acceptance criteria

**AC-1 -- Entity identity has one core-owned interface.**
Verified by: parser item parsing, worktree merge, WorkflowIndex lookup, and OverviewState selection all call the same core identity helper/module for flat `{slug}.md` and folder-form `{slug}/index.md` paths.

**AC-2 -- Existing behavior is preserved for active, archived, and worktree-sourced tasks.**
Verified by: focused tests covering flat files, folder-form `index.md`, archived slug checks, worktree-only items, and selection preservation after reload/sort.

**AC-3 -- TUI code no longer owns workflow schema identity rules.**
Verified by: no TUI-local slug derivation duplicate remains in `crates/spacetop/src/app/overview.rs`; app tests still pass through the public app interface.

**AC-4 -- Read-first product contract remains unchanged.**
Verified by: no new workflow markdown write path is introduced, and `crates/spacetop-core/tests/no_write_git_calls.rs` plus `make lint` pass.

**AC-5 -- Implementation used Ponytail mode.**
Verified by: the stage report explicitly names Ponytail mode/tooling used, or records a blocker if Ponytail mode was unavailable.

## Proof plan

- Lowest test layer: core identity/parser/worktree/index tests first; app tests only for selection behavior.
- Required command: `cargo test -p spacetop-core`, `cargo test -p spacetop app::tests`, and `make lint`.
- Manual check, if any: none expected unless tests reveal a TUI-only selection edge case.
- Docs/policy update needed: update nearby comments only if identity naming changes; no README product behavior change expected.
