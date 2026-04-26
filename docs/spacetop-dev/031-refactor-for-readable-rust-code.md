---
id: "031"
title: Refactor for readable Rust code
status: review
source: user request 2026-04-26
started: 2026-04-26T07:26:44Z
completed:
verdict:
score: 0.62
worktree: .worktrees/spacedock-ensign-031-refactor-for-readable-rust-code
issue:
pr: #21
mod-block: 
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

## Implementation plan

This refactor should proceed in small, reviewable commits or worktree steps that each preserve behavior and keep ownership boundaries unchanged.

1. Establish a behavior baseline.
   - Run `cargo test` before editing so later failures are attributable to the refactor.
   - Run `make lint` before or after the first inspection pass if the branch state is uncertain.
   - Record any pre-existing failures in the implementation report instead of changing expectations to make the refactor pass.

2. Inspect the largest readability hotspots and pick narrow targets.
   - Start with `src/ui/mod.rs`, `src/app.rs`, `src/parser.rs`, and `src/ui/graph.rs`, since they are currently the largest files and contain the most private helpers.
   - Use `rg "^fn |^pub fn |^impl "` and targeted reads to identify long functions, repeated formatting logic, deeply nested branches, or unclear local names.
   - Favor code paths already covered by unit or integration tests; add focused tests only when a touched behavior path has no practical existing coverage.

3. Refactor parser code without changing parsing semantics.
   - Keep workflow README parsing, work item parsing, archive loading, status validation, frontmatter splitting, `.worktrees` scanning, and worktree merge behavior in `src/parser.rs`.
   - Candidate areas: make `load_workflow_dir`, `scan_worktrees`, `merge_worktree_items`, and frontmatter helper flow easier to read by extracting small private helpers or renaming locals that currently hide intent.
   - Do not alter accepted YAML shape, malformed archive skip behavior, active-item ignore rules, worktree merge precedence, or error variants.
   - Verify with existing parser unit tests and `cargo test`; add narrow parser tests only if a helper extraction touches uncovered behavior.

4. Refactor app-state code without introducing terminal rendering concerns.
   - Keep overview sessions, picker state, selection movement, reload semantics, archived scope state, and pending workflow switches in `src/app.rs`.
   - Candidate areas: split long state-transition methods into private helpers with names that describe the decision being made, reduce repeated selected-index bounds logic, and clarify workflow-switch state flow.
   - Do not change key bindings, scope toggling behavior, preview scroll behavior, picker behavior, or reload outcomes.
   - Cover touched behavior with app-state tests where practical rather than relying on a live TUI session.

5. Refactor UI rendering code only after state/parser behavior is stable.
   - Keep terminal rendering in `src/ui/mod.rs`, stage graph rendering in `src/ui/graph.rs`, and picker rendering in `src/ui/picker.rs`.
   - Candidate areas in `src/ui/mod.rs`: decompose preview/header/task-list markdown rendering helpers, clarify layout decisions, and reduce repeated style/span construction when the shared concept is local and obvious.
   - Candidate areas in `src/ui/graph.rs`: clarify width-tier selection, column layout construction, feedback arc collection, and ASCII/Unicode glyph selection without changing rendered content.
   - Preserve footer/help text, keyboard documentation, narrow terminal behavior, Unicode graph/list glyphs, and `SPACETOP_ASCII=1` fallback behavior.
   - Prefer Ratatui `TestBackend` assertions for any rendering path whose structure changes.

6. Refactor discovery, watcher, and launch wiring only if concrete readability wins are visible.
   - Keep scan-root resolution and workflow discovery in `src/discovery.rs`; do not change prune lists or commissioned workflow detection.
   - Keep filesystem watching, event filtering, debounce, backend fallback, and refresh signaling in `src/watcher.rs`; do not change debounce timing or relevant-event criteria.
   - Keep CLI launch decisions, terminal setup, event loop wiring, watcher lifecycle, and top-level `run` behavior in `src/lib.rs`; do not add CLI or runtime behavior.
   - Existing integration tests in `tests/discovery_bypass.rs` and `tests/watcher_fs.rs` should continue to describe the external behavior.

7. Keep each edit behavior-preserving and dependency-neutral.
   - Do not add production dependencies, CLI options, key bindings, parsing contracts, UI features, telemetry behavior, or workflow write support.
   - Avoid broad file moves or public API reshaping unless the current module already exposes that boundary.
   - Prefer private helpers, clearer local names, reduced nesting, and explicit error-path names over new abstractions.

8. Finish with verification and an implementation report.
   - Run `cargo fmt`.
   - Run `cargo test`.
   - Run `make lint`.
   - If watcher behavior changes, additionally run `cargo test -- --ignored` locally and note that it exercises the real `notify` backend.
   - In the implementation report, name the exact functions or private types simplified, the tests that cover them, and confirm no feature, dependency, or module-ownership expansion occurred.

## Stage Report: plan

- DONE: Produce a concrete step-by-step implementation plan that identifies likely code areas to inspect and refactor while preserving behavior.
  Added an eight-step implementation plan naming `src/ui/mod.rs`, `src/app.rs`, `src/parser.rs`, `src/ui/graph.rs`, `src/discovery.rs`, `src/watcher.rs`, and `src/lib.rs` as scoped inspection/refactor areas.
- DONE: Include a focused verification strategy with exact commands, including `cargo test` and `make lint`, and note where lower-layer tests should cover touched behavior.
  The plan requires baseline and final `cargo test`, final `make lint`, `cargo fmt`, optional `cargo test -- --ignored` for watcher changes, and lower-layer parser/app/UI test coverage for touched behavior.
- DONE: Include file/module ownership guidance that keeps parser, discovery, watcher, app-state, launch, and UI concerns in their existing modules and forbids feature/dependency expansion.
  The plan explicitly keeps parser logic in `src/parser.rs`, discovery in `src/discovery.rs`, watcher logic in `src/watcher.rs`, app state in `src/app.rs`, launch wiring in `src/lib.rs`, and rendering in `src/ui/`, while forbidding new features or dependencies.

### Summary

Planned a no-feature-change readability refactor with phased inspection, narrow module-local edits, and verification gates. The plan emphasizes private helper extraction, clearer naming, lower-layer tests for touched behavior, and preserving all documented SpaceTop module ownership boundaries.

## Stage Report: implement

- DONE: Make a concrete readability refactor in production Rust code that preserves behavior and improves named functions or helper flow.
  Refactored `src/parser.rs` by extracting `collect_active_item_paths`, `collect_archived_item_paths`, `read_directory`, `load_worktree_items`, `collect_worktree_item_paths`, `merged_worktree_item`, `content_hash`, and `merge_main_frontmatter_with_worktree_body` from `load_archived_items`, `load_workflow_dir`, `scan_worktrees`, and `merge_worktree_items`.
- DONE: Run `cargo fmt`, `cargo test`, and `make lint`, and report exact outcomes; if any command cannot pass, explain the blocker with evidence.
  Baseline `cargo test` passed 174 lib tests, 4 main tests, 8 integration tests, 0 doctests, with 1 ignored watcher test; baseline `make lint` passed. Final `cargo fmt` completed, final `cargo test` passed the same non-ignored suites, and final `make lint` passed.
- DONE: Append an implementation stage report naming files/functions changed, tests run, and confirming no feature/dependency/module-ownership expansion.
  This report names the parser helpers changed, records `cargo fmt`, `cargo test`, and `make lint`, and confirms changes stayed in parser ownership with no new features, dependencies, CLI flags, key bindings, UI behavior, telemetry behavior, parsing contracts, or write support.

### Summary

Implemented a narrow parser readability refactor in `src/parser.rs`. The change keeps active item loading, archive loading, `.worktrees` scanning, and worktree merge precedence behavior-covered by existing parser and integration tests while reducing nesting and making path collection and merge decisions explicit.

## Stage Report: review

- DONE: Inspect the implementation diff and confirm whether the parser refactor preserves behavior and improves readability without expanding scope.
  Verdict: PASSED. Reviewed `6cfd456` diff; `src/parser.rs` helper extraction preserves active/archive/worktree parsing and merge precedence while making path collection and hash/body merge flow explicit.
- DONE: Check the implementation evidence against all five acceptance criteria and identify any missing or weak evidence.
  `cargo test` passed 174 lib tests, 4 main tests, 8 integration tests, 0 doctests, with 1 ignored watcher test; `make lint` passed; `git diff --check main...HEAD` passed. `cargo fmt --check` is not clean, but the same rustfmt drift reproduces on main and is not introduced by this branch.
- DONE: Append a gated review report with a clear `PASSED` or `REJECTED` verdict and commit it.
  Appended this review report to the worktree entity file only; no production Rust code or tests were modified during review.

### Summary

PASSED. The implementation stays in parser ownership, adds no feature/dependency/CLI/key binding/UI/telemetry/parsing-contract/workflow-write expansion, and names concrete readability improvements in `load_workflow_dir`, `load_archived_items`, `scan_worktrees`, and `merge_worktree_items`. The only caveat is weak fmt evidence in the implementation report because current `cargo fmt --check` is repo-wide dirty on both main and the review worktree, but required behavior and lint gates pass.

## Stage Report: implement follow-up

- DONE: Inspect the whole Rust codebase and record module-by-module findings, including modules intentionally left unchanged.
  Inspected `src/app.rs` (changed key handling flow), `src/lib.rs` (changed launch decision duplication), `src/discovery.rs` (changed walk/readme decision helpers), `src/watcher.rs` (changed event forwarding/debounce first-event helpers), `src/ui/mod.rs` (changed footer hint construction), `src/ui/graph.rs` (changed feedback-line helpers), `src/ui/picker.rs` (changed picker row helper), `src/domain/mod.rs` (changed color constants/hue helper), `src/main.rs` (changed Sentry init/capture helpers), `src/cli.rs` (left unchanged because the small clap surface was already clear and covered), `src/parser.rs` (left unchanged in this follow-up because the prior accepted helper extraction already covered parser ownership), and `tests/discovery_bypass.rs` plus `tests/watcher_fs.rs` (left unchanged because existing integration assertions already covered the refactored launch/discovery/watcher behavior).
- DONE: Broaden the readability refactor with behavior-preserving changes in multiple modules where concrete readability wins exist.
  Broadened production changes across app, launch, discovery, watcher, UI, graph, picker, domain, and main modules using private helpers, clearer decision names, and reduced nesting without adding public API surface or behavior.
- DONE: Run `cargo fmt`, `cargo test`, `make lint`, and `git diff --check`, and report exact outcomes.
  `cargo fmt` completed; final `cargo test` passed 174 lib tests, 4 main tests, 8 integration tests, and 0 doctests with 1 ignored watcher test; final `make lint` passed via `cargo clippy --all-targets --all-features -- -D warnings`; final `git diff --check` exited cleanly.
- DONE: Confirm no feature/dependency/module-ownership expansion and no behavior changes.
  No dependencies, CLI flags, key bindings, UI behavior, parsing contracts, telemetry behavior, workflow writes, or module ownership boundaries were expanded; all changes are private helper/readability refactors in their existing modules.

### Summary

Followed up on the gate rejection by inspecting the required codebase areas and broadening the refactor beyond `src/parser.rs`. The follow-up keeps behavior covered by the existing app, discovery, watcher, UI, main, and integration tests while documenting why `src/cli.rs`, `src/parser.rs`, and integration test files did not need further code changes in this pass.

## Stage Report: review follow-up

- DONE: Inspect the broadened codebase-wide implementation diff and confirm whether it satisfies the captain's feedback, not just the original parser slice.
  Verdict: PASSED. Reviewed `main...HEAD`, including parser refactor commits and `5ce8683`; production changes now span `src/app.rs`, `src/lib.rs`, `src/discovery.rs`, `src/watcher.rs`, `src/ui/mod.rs`, `src/ui/graph.rs`, `src/ui/picker.rs`, `src/domain/mod.rs`, `src/main.rs`, and `src/parser.rs`.
- DONE: Check behavior preservation, module ownership, and no feature/dependency expansion across all changed modules.
  The diff is private helper extraction, clearer local decision helpers, and rustfmt-only test wrapping inside touched production modules; `Cargo.toml`, `Cargo.lock`, CLI flags, key bindings, parser contracts, UI behavior, telemetry behavior, and workflow write behavior are unchanged.
- DONE: Verify or credibly audit the reported `cargo fmt`, `cargo test`, `make lint`, and `git diff --check` evidence.
  `cargo test` passed 174 lib tests, 4 main tests, 8 integration tests, 0 doctests, with 1 ignored watcher test; `make lint` passed; `git diff --check main...HEAD` passed; `cargo fmt --check` reports only unchanged `tests/discovery_bypass.rs` formatting drift, so the implementation's `cargo fmt` evidence remains weak but not branch-introduced.
- DONE: Append a gated review follow-up report with a clear `PASSED` or `REJECTED` verdict and commit it.
  Appended this review follow-up report to the worktree entity file only; no production Rust code or tests were modified during review.

### Summary

PASSED. The broadened follow-up satisfies the captain's scope correction by inspecting and refactoring across the Rust codebase while leaving `src/cli.rs` and integration tests justifiably unchanged. No defects were found; the only caveat is that repo-wide `cargo fmt --check` still fails on an unchanged integration-test formatting line, so future cleanup should normalize that file separately.
