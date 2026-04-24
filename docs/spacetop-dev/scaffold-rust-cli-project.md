---
id: 001
title: Scaffold Rust CLI Project
status: design
source: commission seed
started:
completed:
verdict:
score: 0.9
worktree:
issue:
pr:
---

Create the initial Rust crate, command entrypoint, dependency baseline, and basic executable structure for SpaceTop. The scaffold should establish a clean place for CLI argument parsing, domain parsing, application state, and terminal UI code.

## Acceptance criteria

**AC-1 -- The project has a runnable Rust CLI entrypoint.**
Verified by: `cargo run -- --help` exits successfully and displays SpaceTop-oriented usage.

**AC-2 -- Core dependencies are intentionally chosen.**
Verified by: `Cargo.toml` includes the selected CLI/TUI/parsing crates with no unused placeholder dependencies.

**AC-3 -- Basic Rust quality gates pass.**
Verified by: `cargo fmt --check` and `cargo test` pass.
