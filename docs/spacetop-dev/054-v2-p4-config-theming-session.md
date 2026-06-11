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

## Stage Report: implement

- DONE: Added core YAML config model, default theme/keybinding/default-scope values, XDG/HOME path resolution, config loading, and non-fatal malformed-config warnings.
  Evidence: `cargo test -p spacetop-core config::tests` passed, 9 passed.
- DONE: Added typed session state under safe user state paths, YAML round-trip load/save, and canonical absolute workflow session keys.
  Evidence: `cargo test -p spacetop-core session_state::tests` passed, 9 passed.
- DONE: Applied config at TUI startup before session restore; malformed config and keymap warnings are stored on `App` and rendered in the footer warning surface.
  Evidence: full workspace tests passed, including app warning/default/session precedence tests.
- DONE: Added configurable theme colors at the UI boundary with hex parsing and deterministic fallback to existing colors.
  Evidence: workspace tests passed, including `ui::color` and configured footer/task-list color tests.
- DONE: Added validated configurable P3 keybindings with duplicate, invalid, and reserved-key fallback warnings; help and footer labels render from the resolved keymap.
  Evidence: workspace tests passed, including configured search, duplicate, reserved, help-label, and footer-label tests.
- DONE: Persisted selected entity and active/archived scope per canonical workflow key on clean exit, with session state overriding config defaults.
  Evidence: workspace tests passed, including session restore/save and no-write git guard tests.
- DONE: Updated README, AGENTS, and development policy with config/session paths and the safety boundary that config/session writes are only under user config/state paths, not workflow markdown.

### Proof Commands

- `cargo test -p spacetop-core config::tests` -> passed, 9 passed.
- `cargo test -p spacetop-core session_state::tests` -> passed, 9 passed.
- `cargo test --workspace` -> passed: spacetop lib 311 passed; main 4 passed; integration tests 10/4/5 passed; spacetop-core lib 144 passed; core integration tests 7/1/2 passed; watcher real-backend tests 3 ignored; doctests 0.
- `make lint` -> passed, `cargo clippy --all-targets --all-features -- -D warnings`.
- `cargo test -p spacetop-core --test no_write_git_calls` -> passed, 2 passed.

### Commits

- `2461ad8 feat(core): add config model and XDG loading`
- `a3d1059 feat(core): add XDG session persistence model`
- `2cb3708 feat(tui): load and carry user config`
- `638dfca feat(tui): apply configured theme colors`
- `af90d2a feat(tui): honor configured keybindings`
- `f75a5e3 feat(tui): persist per-workflow session state`
- `78f6e01 docs: document config and session persistence paths`
- `f2a40ee fix(core): derive simple defaults`

### Notes

- Subagent spawning was not available in this runtime, so I followed the required fallback locally: TDD by slice, focused tests before integration tests, scoped commits, and self-review.
- No workflow markdown was modified except this implement-stage report. Config/session writes are constrained to absolute XDG/HOME-derived user paths.
