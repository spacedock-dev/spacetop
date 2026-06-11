---
id: "051"
title: "v2 P1: index and query API"
status: verify
source: "captain - reviewed SpaceTop v2 roadmap plan"
kind: refactor
risk: high
milestone: v2-p1
proof: "cargo test --workspace; make lint; cargo test -p spacetop-core --test no_terminal_deps"
started: 2026-06-11
completed:
verdict:
score: 0.98
worktree: .worktrees/spacedock-ensign-051-v2-p1-index-query-api
issue:
pr: "#51"
mod-block: merge:pr-merge
---

Implement phase P1 of the SpaceTop v2 internals rebuild: introduce
`WorkflowIndex`, terminal-free query DTOs, source wrappers, and query-backed TUI
accessors while preserving existing list, preview, graph, archive, sync, picker,
and help behavior.

Plan: `docs/superpowers/plans/2026-06-11-spacetop-v2-p1-index-query-api.md`

This entity was fired without dispatch at the captain's request.

## Scope

- Kind: refactor
- Risk: high
- Milestone: v2-p1
- Touches: core index/query, source loading, parser archive behavior, app-state, UI rendering, docs
- Non-goals: git history ingestion, new TUI views, config/session persistence, headless CLI

## Acceptance criteria

**AC-1 -- Core query spine exists and remains terminal-free.**
`spacetop-core` owns `WorkflowIndex`, `EntityQuery`, typed field filters, owned query results, and serializable DTOs without terminal dependencies.
Verified by: core query/index tests and `cargo test -p spacetop-core --test no_terminal_deps`.

**AC-2 -- Existing TUI behavior is preserved through query-backed state.**
List, preview, graph/header counts, archive scope, sort, picker, sync, and help render from query-backed accessors without visible behavior drift.
Verified by: app tests plus Ratatui UI tests named in the plan.

**AC-3 -- Archive behavior remains lazy and auditable.**
Startup does not read `_archive/`; archive scope explicitly loads archive data, preserves archive ordering, and surfaces archive parse errors only after archive scope is loaded.
Verified by: app archive tests and parser/archive tests.

**AC-4 -- Reload still rebuilds a fresh index.**
Watcher and README reload paths replace index contents without incremental update logic.
Verified by: reload tests and `cargo test --workspace`.

## Proof plan

- Lowest test layer: core query/parser tests first, then app and Ratatui render tests.
- Required commands: `cargo test --workspace`; `make lint`; `cargo test -p spacetop-core --test no_terminal_deps`.
- Manual check, if any: optional TUI smoke only if render behavior appears ambiguous.
- Docs/policy update needed: update code maps for `index.rs`, `query.rs`, and `sources.rs`.

## Stage Report: implement

### Checklist

- DONE - Build the terminal-free core query/index spine: `WorkflowIndex`, owned `EntityQuery` results, serializable DTOs, source wrappers, and no terminal dependencies in `spacetop-core`.
- DONE - Preserve existing TUI behavior through query-backed app/UI accessors, including list, preview, graph/header counts, archive scope, sort, picker, sync, and help behavior.
- DONE - Prove archive/reload semantics and the read-only boundary: lazy archive loading, fresh index rebuilds on reload, focused tests, docs/code-map updates, `cargo test --workspace`, `make lint`, and `cargo test -p spacetop-core --test no_terminal_deps`.

### Summary

Implemented the v2 P1 index/query spine in `spacetop-core`, including serializable workflow DTOs, `EntityQuery` filtering/sorting, source wrappers, and full-rebuild `WorkflowIndex` loading. Migrated overview/app/UI accessors to read through the index while preserving existing list, preview, graph, archive, sort, picker, sync, and help behavior. Archive loading remains lazy, archive parse errors surface only after archive scope loads, and reload replaces the index instead of attempting incremental updates.

### Commands And Outcomes

- PASS - `cargo test --workspace`
  - `spacetop` lib: 257 passed.
  - `spacetop` main: 4 passed.
  - Integration suites: `discovery_bypass` 10 passed, `git_sync_e2e` 4 passed, `readme_reload` 5 passed.
  - `spacetop-core` lib: 109 passed.
  - Guard tests: `no_terminal_deps` 1 passed, `no_write_git_calls` 2 passed.
  - `watcher_fs`: 3 ignored by design because they exercise the real notify backend.
- PASS - `make lint`
  - Ran `cargo clippy --all-targets --all-features -- -D warnings`.
- PASS - `cargo test -p spacetop-core --test no_terminal_deps`
  - `core_dependency_tree_has_no_terminal_crates` passed.

### Known Failures

- FIXED - An earlier `make lint` run failed on `clippy::large-enum-variant` after `SelectedRow` began owning `Entity`; fixed by boxing the `SelectedRow::Item` payload.
- FIXED - Earlier workspace test runs exposed stale assertions that hard-coded the old `design`/`review` stage names from `docs/spacetop-dev`; fixed by aligning parser tests with current `shape`/`verify` metadata and making UI graph/definition tests derive real stage names from the loaded definition.

### Read-Only Boundary

No workflow-state write path was added. The only workflow markdown edit in this stage is this dispatched stage report appended to the entity file.
