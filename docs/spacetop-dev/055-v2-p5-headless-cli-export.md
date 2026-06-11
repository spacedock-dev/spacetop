---
id: "055"
title: "v2 P5: headless CLI and export"
status: verify
source: "captain - reviewed SpaceTop v2 roadmap plan"
kind: feature
risk: medium
milestone: v2-p5
proof: "cargo test --workspace; make lint; cargo run -p spacetop -- list --workflow-dir docs/spacetop-dev --json"
started: 2026-06-11
completed:
verdict:
score: 0.8
worktree: .worktrees/spacedock-ensign-055-v2-p5-headless-cli-export
issue:
pr: "#55"
mod-block: merge:pr-merge
---

Implement phase P5 of the SpaceTop v2 internals rebuild: add headless CLI
subcommands and JSON export over the core query API while preserving current
no-argument TUI launch behavior.

Plan: `docs/superpowers/plans/2026-06-11-spacetop-v2-p5-headless-cli-export.md`

This entity was fired without dispatch at the captain's request.

## Scope

- Kind: feature
- Risk: medium
- Milestone: v2-p5
- Touches: clap CLI, headless workflow resolution, query/export output, config defaults, README/development policy
- Non-goals: separate binary, new workflow writes, TUI behavior changes, third crate split unless justified by measured thresholds

## Acceptance criteria

**AC-1 -- Existing TUI launch behavior is unchanged.**
`spacetop` and `spacetop --workflow-dir ...` still launch the TUI as before.
Verified by: CLI parse tests and smoke command.

**AC-2 -- Headless commands resolve exactly one workflow.**
Direct workflow paths, repo/root paths, and omitted paths use discovery and reject zero or multiple workflows with stable errors.
Verified by: headless resolver tests.

**AC-3 -- Headless output uses core query/history APIs and P4 config defaults.**
`list`, `timeline`, `metrics`, `activity`, and `export --json` output data or stable unavailable responses without placeholder command paths.
Verified by: headless command tests with injected git runner failures.

**AC-4 -- Export is serializable and auditable.**
JSON export includes definition, active entities, and archived entities by attaching archive data explicitly.
Verified by: export serialization tests and CLI JSON smoke commands.

**AC-5 -- Third-crate split decision is evidence-backed.**
The plan either defers the split with Decision Tabs and measurements, or justifies it against documented thresholds.
Verified by: recorded build/size evidence and docs update.

## Proof plan

- Lowest test layer: CLI parse and headless resolver tests, then command output tests.
- Required commands: `cargo test --workspace`; `make lint`; `cargo run -p spacetop -- list --workflow-dir docs/spacetop-dev --json`; `cargo run -p spacetop -- export --workflow-dir docs/spacetop-dev --json`.
- Manual check, if any: no manual TUI check unless launch behavior changes unexpectedly.
- Docs/policy update needed: README CLI examples and crate-split decision note.
