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
pr:
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

## Stage Report: implement

- DONE: AC-1 existing TUI launch behavior is unchanged.
  Evidence: `Cli` keeps top-level `workflow_dir` and uses `command: Option<Command>`; `cli::tests::no_subcommand_still_launches_tui_shape` covers `spacetop --workflow-dir ...` with no subcommand; existing `decide_app` integration tests still pass. Headless dispatch runs only when `cli.command` is `Some`.
- DONE: AC-2 headless commands resolve exactly one workflow.
  Evidence: `headless::resolve_workflow_arg` canonicalizes direct workflow paths, explicit scan roots, and omitted cwd, then requires exactly one discovered workflow. Tests cover direct workflow path, one-workflow scan root, zero workflows, and multiple workflows with the stable `headless command requires exactly one workflow` error.
- DONE: AC-3 headless output uses core query/history APIs and P4 config defaults.
  Evidence: `list` loads `WorkflowIndex`, attaches archives for archived/all scopes, queries with `EntityQuery`, and applies config default scope/sort unless CLI scope overrides. `timeline`, `metrics`, and `activity` use `WorkflowIndex::load_with_history` with injectable `GitRunner` tests for shallow clone, non-git, and generic git errors in JSON and text modes.
- DONE: AC-4 export is serializable and auditable.
  Evidence: `export --json` requires `--json`, attaches archive data explicitly with `WorkflowSources::load_archive`, and serializes `{ definition, entities, archived_entities }`. Tests assert definition, active entity, and archived entity output.
- DONE: AC-5 third-crate split decision is evidence-backed.
  Evidence: ran `cargo clean`; clean `cargo build -p spacetop` took 25.93s; `cargo build -p spacetop --release` took 48.81s; `target/release/spacetop` size was 9.4M. Decision Tabs in `docs/development-policy.md` keep the P0 two-crate workspace and defer a `spacetop-tui` split until measured build/artifact pressure or a downstream TUI-free binary need exists.
- DONE: Required proof commands passed.
  Evidence:
  - `cargo test --workspace` -> passed after clippy fix; spacetop lib 333 passed, main 4 passed, spacetop integration tests 10/4/5 passed, spacetop-core lib 145 passed, core integration tests 7/1/2 passed, watcher real-backend tests 3 ignored, doctests 0.
  - `make lint` -> passed after changing headless map text rendering to `sort_by_key`; `cargo clippy --all-targets --all-features -- -D warnings`.
  - `cargo run -p spacetop -- list --workflow-dir docs/spacetop-dev --json` -> passed, exited 0, emitted active task JSON.
  - `cargo run -p spacetop -- export --workflow-dir docs/spacetop-dev --json` -> passed, exited 0, emitted JSON with `definition`, `entities`, and `archived_entities`.

### Commit List

- `d735a37` feat(cli): define headless list command
- `a26b226` feat(cli): resolve one workflow for headless commands
- `f67c99b` feat(cli): add headless list command
- `72be316` feat(cli): add headless history and metrics commands
- `9c36d83` feat(cli): add JSON export command
- `aac13b8` test(cli): pin headless resolver and config defaults
- `7c03963` docs: document headless CLI surface and crate split decision
- `d208599` fix(cli): satisfy clippy for headless text output

### Summary

Implemented the P5 headless CLI surface in the existing `spacetop` binary while keeping `spacetop-core` terminal-free and preserving the current TUI launch shape. The implementation adds tested one-workflow resolution, query-backed list output, history-backed timeline/metrics/activity commands with stable unavailable responses, JSON export with archived entities, README examples, and a measured no-split decision.
