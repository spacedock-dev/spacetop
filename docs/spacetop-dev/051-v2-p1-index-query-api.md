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
