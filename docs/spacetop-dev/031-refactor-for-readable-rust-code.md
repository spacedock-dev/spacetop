---
id: "031"
title: Refactor for readable Rust code
status: design
source: user request 2026-04-26
started:
completed:
verdict:
score: 0.62
worktree:
issue:
pr:
---

Refactor the SpaceTop Rust codebase to make the implementation easier for future maintainers to read, navigate, and change while preserving the current product behavior.

This task must not add, remove, or modify user-facing features. The goal is internal clarity: code should better reflect Rust coding guidelines, the repository's existing module ownership boundaries, and clean code principles such as clear naming, focused functions, low surprise, explicit data flow, and testable behavior.

## Scope

The refactor should focus on production Rust code and directly related tests where they help prove behavior stayed unchanged.

Good candidates include:

- Simplifying long or deeply nested functions without changing their observable behavior.
- Improving names for local variables, helper functions, or private types when current names obscure intent.
- Reducing duplication where the shared concept is already clear and the abstraction remains local and obvious.
- Moving parsing, app-state, watcher, or UI-specific logic back behind the module boundary documented in `AGENTS.md` when it has drifted.
- Replacing ad hoc control flow with idiomatic Rust constructs where that makes behavior clearer.
- Tightening error handling so failure paths remain explicit and easy to follow.

Out of scope:

- New CLI flags, TUI controls, workflow parsing semantics, rendering behavior, dependencies, telemetry behavior, or write support.
- Cosmetic churn that only renames or reshuffles code without making intent clearer.
- Broad architectural rewrites that make review risky or require unrelated behavior changes.

## Acceptance criteria

Each AC names a property of the finished entity, not a stage action.

**AC-1 -- Public behavior is unchanged.**
Verified by: `cargo test` and focused tests for any touched parser, app-state, watcher, discovery, or UI rendering behavior continue to pass without expectation changes that broaden or alter behavior.

**AC-2 -- Lint gate is clean.**
Verified by: `make lint` completes successfully with clippy warnings denied.

**AC-3 -- Refactoring stays within existing module ownership.**
Verified by: review of touched files shows parser assumptions remain in `src/parser.rs`, discovery logic in `src/discovery.rs`, watcher logic in `src/watcher.rs`, app state in `src/app.rs`, launch wiring in `src/lib.rs`, and terminal rendering in `src/ui/`.

**AC-4 -- Readability improves in concrete code paths.**
Verified by: the implementation report names the specific functions or types simplified, and the diff shows reduced nesting, clearer names, smaller helpers, or removed duplication without adding a large new abstraction.

**AC-5 -- No feature or dependency expansion occurs.**
Verified by: review confirms no new CLI options, keyboard behavior, parsing contracts, UI features, workflow write behavior, or production dependencies were introduced.
