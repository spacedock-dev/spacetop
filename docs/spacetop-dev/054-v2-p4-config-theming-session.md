---
id: "054"
title: "v2 P4: config, theming, and session persistence"
status: verify
source: "captain - reviewed SpaceTop v2 roadmap plan"
kind: feature
risk: medium
milestone: v2-p4
proof: "cargo test --workspace; make lint; cargo test -p spacetop-core --test no_write_git_calls"
started: 2026-06-11
completed:
verdict:
score: 0.84
worktree: .worktrees/spacedock-ensign-054-v2-p4-config-theming-session
issue:
pr:
mod-block: merge:pr-merge
---

Implement phase P4 of the SpaceTop v2 internals rebuild: add YAML config,
theme defaults, validated configurable keybindings, and per-workflow session
persistence under user config/state paths.

Plan: `docs/superpowers/plans/2026-06-11-spacetop-v2-p4-config-theming-session.md`

This entity was fired without dispatch at the captain's request.

## Scope

- Kind: feature
- Risk: medium
- Milestone: v2-p4
- Touches: core config/session models, TUI app startup, key handling, UI colors, docs/policy
- Non-goals: workflow-local config, workflow markdown writes, headless CLI implementation

## Acceptance criteria

**AC-1 -- Config and session paths are safe.**
Config/session files load from absolute XDG/HOME-derived user paths only and never write into workflow directories by accident.
Verified by: config/session path tests, including relative XDG fallback cases.

**AC-2 -- Bad config is visible but non-fatal.**
Malformed config falls back to defaults while preserving user-visible warnings.
Verified by: config load tests and app/footer warning tests.

**AC-3 -- Keybindings resolve through a validated keymap.**
Duplicate, invalid, or reserved keybindings fall back to defaults and surface warnings; help/footer labels use the resolved keymap.
Verified by: key handling tests and help/footer render tests.

**AC-4 -- Session state is typed and stable per workflow.**
Selected entity and scope persist by canonical workflow key, with session state overriding config defaults when both exist.
Verified by: session round-trip tests and app restore tests.

## Proof plan

- Lowest test layer: core config/session tests first, then app/key and UI color tests.
- Required commands: `cargo test --workspace`; `make lint`; `cargo test -p spacetop-core --test no_write_git_calls`.
- Manual check, if any: optional TUI smoke for theme/key labels.
- Docs/policy update needed: README, AGENTS, and development policy config/session boundaries.
