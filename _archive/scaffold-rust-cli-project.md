---
id: 001
title: Scaffold Rust CLI Project
status: done
source: commission seed
started: 2026-04-24T14:30:53Z
completed: 2026-04-24T14:49:53Z
verdict: PASSED
score: 0.9
worktree: 
issue:
pr:
mod-block: 
archived: 2026-04-24T14:49:53Z
---

Create the initial Rust crate, command entrypoint, dependency baseline, and basic executable structure for SpaceTop. The scaffold should establish a clean place for CLI argument parsing, domain parsing, application state, and terminal UI code.

## Acceptance criteria

**AC-1 -- The project has a runnable Rust CLI entrypoint.**
Verified by: `cargo run -- --help` exits successfully and displays SpaceTop-oriented usage.

**AC-2 -- Core dependencies are intentionally chosen.**
Verified by: `Cargo.toml` includes the selected CLI/TUI/parsing crates with no unused placeholder dependencies.

**AC-3 -- Basic Rust quality gates pass.**
Verified by: `cargo fmt --check` and `cargo test` pass.

## Implementation Plan

1. Create a Rust binary crate at the repository root with `Cargo.toml`, `Cargo.lock`, and `src/` files.
   - Use package name `spacetop`, Rust 2021 edition, and a single CLI binary.
   - Keep the root `README.md` and existing workflow files unchanged except for any later documentation task.

2. Establish the dependency baseline in `Cargo.toml`.
   - Runtime dependencies: `clap` with `derive` for CLI parsing, `ratatui` for terminal rendering, `crossterm` for terminal backend/events, `serde` with `derive` for typed metadata, `serde_yaml` for YAML frontmatter, `anyhow` for application-level error context, and `thiserror` for domain/parser error types once parsing modules are introduced.
   - Dev dependencies: only add a crate when a test needs it; the scaffold itself should rely on standard Rust tests.
   - Do not add placeholder dependencies for future features such as markdown rendering, git inspection, or async IO until an implementation task uses them.

3. Add the CLI entrypoint and argument boundary.
   - Create `src/main.rs` as the thin executable entrypoint.
   - Create `src/cli.rs` with a `Cli` struct using `clap::Parser`.
   - Support `cargo run -- --help` with SpaceTop-oriented usage text and an optional `--workflow-dir <PATH>` argument that defaults to the current directory when omitted.
   - Keep command execution in a small `run(cli: Cli) -> anyhow::Result<()>` function so tests can exercise non-terminal behavior later.

4. Add module boundaries without implementing full parsing or TUI behavior in this task.
   - Create `src/lib.rs` exporting `cli`, `app`, `domain`, and `ui` modules.
   - Create `src/app.rs` for application state and future orchestration.
   - Create `src/domain/mod.rs` for future Spacedock workflow parsing types.
   - Create `src/ui/mod.rs` for future ratatui rendering and input boundaries.
   - Keep terminal setup out of domain/state modules so parsing and state logic remain testable without a TUI backend.

5. Add minimal scaffold behavior.
   - `main` should parse CLI arguments and call `spacetop::run`.
   - `run` can currently validate/store the selected workflow path and return successfully without mutating files.
   - Avoid opening a terminal alternate screen or starting an event loop in this scaffold task; that belongs to the initial TUI task.

6. Add focused tests for the scaffold.
   - Unit-test `Cli::command().debug_assert()` so the Clap definition is internally valid.
   - Unit-test that parsing `["spacetop", "--workflow-dir", "docs/spacetop-dev"]` records the expected path.
   - Unit-test that parsing `["spacetop"]` succeeds and uses the documented current-directory default.
   - Add a lightweight test around the initial app state constructor if one is introduced.

7. Verify the scaffold with reproducible commands.
   - Run `cargo fmt --check`.
   - Run `cargo test`.
   - Run `cargo run -- --help` and confirm it exits successfully with SpaceTop-oriented usage and the `--workflow-dir` option.
   - Optional implementation evidence: `cargo check` can be run during development, but the stage acceptance evidence should include the three commands above.

## File and Module Ownership for Implementation

- Owned by this task's implementation worker: `Cargo.toml`, `Cargo.lock`, `src/main.rs`, `src/lib.rs`, `src/cli.rs`, `src/app.rs`, `src/domain/mod.rs`, and `src/ui/mod.rs`.
- Shared/protected surfaces: do not modify `docs/spacetop-dev/*.md` task files, `docs/spacetop-dev/README.md`, or existing workflow state files during implementation unless the first officer dispatches that explicitly.
- Follow-on task boundaries: full Spacedock markdown/frontmatter parsing belongs to `parse-spacedock-workflow-files.md`; terminal layout, rendering, and event handling belong to `build-initial-tui-overview.md`.
- Read-only safety: the scaffold must not write to workflow directories or mutate markdown state.

## Stage Report: plan

- DONE: DONE/SKIPPED/FAILED accounting must show a concrete implementation plan for creating the Rust crate, CLI entrypoint, and dependency baseline.
  Added `Implementation Plan` steps covering crate creation, `Cargo.toml` dependencies, CLI entrypoint, module layout, scaffold behavior, tests, and verification commands.
- DONE: DONE/SKIPPED/FAILED accounting must show a focused scaffold test/verification strategy.
  Added focused Clap/app scaffold tests plus required verification commands: `cargo fmt --check`, `cargo test`, and `cargo run -- --help`.
- DONE: DONE/SKIPPED/FAILED accounting must identify file/module ownership for the later implementation stage.
  Added `File and Module Ownership for Implementation` with owned Rust scaffold files and protected workflow surfaces.

### Summary

Planned the root Rust binary scaffold with a thin `main`, testable CLI boundary, minimal app state boundary, and placeholder module structure for domain and UI work. The plan keeps parsing/state logic separate from terminal rendering and leaves workflow markdown read-only by default.

## Stage Report: implement

- DONE: DONE/SKIPPED/FAILED accounting must show a runnable Rust CLI entrypoint with SpaceTop-oriented `--help` and `--workflow-dir` parsing.
  `src/main.rs` parses `Cli` and calls `spacetop::run`; `cargo run -- --help` exits 0 and shows SpaceTop usage plus `--workflow-dir <PATH>`.
- DONE: DONE/SKIPPED/FAILED accounting must show an intentional dependency baseline in `Cargo.toml` and committed `Cargo.lock`.
  `Cargo.toml` adds clap, ratatui, crossterm, serde, serde_yaml, anyhow, and thiserror; `Cargo.lock` was generated by Cargo in this worktree.
- DONE: DONE/SKIPPED/FAILED accounting must show `cargo fmt --check`, `cargo test`, and `cargo run -- --help` evidence.
  `cargo fmt --check` passed; `cargo test` passed with 5 tests; `cargo run -- --help` passed and displayed workflow-dir help.

### Summary

Implemented the initial Rust 2021 `spacetop` crate with a thin binary entrypoint, testable CLI parsing, minimal app state, and placeholder domain/UI boundaries. The scaffold does not start a terminal event loop and does not mutate workflow markdown.

## Stage Report: review

- DONE: DONE/SKIPPED/FAILED accounting must show whether AC-1 (`cargo run -- --help`) is satisfied with fresh evidence.
  Fresh `cargo run -- --help` exited 0 and displayed SpaceTop read-only TUI usage with `--workflow-dir <PATH>` and default `.`.
- DONE: DONE/SKIPPED/FAILED accounting must show whether AC-2 (`Cargo.toml` dependency baseline) is satisfied by inspecting the dependency set.
  Inspected `Cargo.toml`: dependencies are clap, ratatui, crossterm, serde, serde_yaml, anyhow, and thiserror, each referenced by scaffold modules.
- DONE: DONE/SKIPPED/FAILED accounting must show whether AC-3 (`cargo fmt --check` and `cargo test`) is satisfied with fresh evidence.
  Fresh `cargo fmt --check` exited 0; fresh `cargo test` exited 0 with 5 passed, 0 failed.

### Summary

Verdict: PASSED. Reviewed the diff against `main`, checked the Rust scaffold boundaries, and independently reran the claimed acceptance commands. No blocking defects found; the scaffold is read-only, keeps terminal setup out of domain/app state, and leaves full parsing/TUI behavior to follow-on tasks.
