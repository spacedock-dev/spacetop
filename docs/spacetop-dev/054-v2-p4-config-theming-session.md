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
pr: "#54"
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

## Stage Report: verify

- DONE: AC-1 config and session paths are safe. Evidence: `crates/spacetop-core/src/config.rs` resolves config only from absolute `XDG_CONFIG_HOME` or absolute `HOME` fallback, with tests for relative and empty XDG plus relative HOME; `crates/spacetop-core/src/session_state.rs` does the same for `XDG_STATE_HOME` and rejects relative session file paths before load/save. Targeted tests passed: `cargo test -p spacetop-core config::tests` (9 passed), `cargo test -p spacetop-core session_state::tests` (9 passed), and `cargo test -p spacetop save_session_state_for_app` (2 passed).
- DONE: AC-2 malformed config is visible but non-fatal. Evidence: malformed YAML returns default config with a `failed to parse config` warning in `load_config_file_with_warnings`, and UI tests render config/keymap warnings in the footer. Targeted evidence: `cargo test -p spacetop-core config::tests` and `cargo test -p spacetop ui::tests` both passed.
- FAILED: AC-3 keybindings do not always resolve to a validated final keymap. Evidence: `ResolvedKeymap::from_config` counts duplicates only across parsed configured keys before invalid/reserved/duplicate entries fall back to defaults (`crates/spacetop/src/app/keys.rs:52-84`). A config such as `search: ""` or `search: "a"` plus `command: "/"` resolves both search and command to `/`; input matching checks search before command (`crates/spacetop/src/app/keys.rs:291-295`), so command becomes unreachable and no duplicate warning is produced for the final keymap. Existing key tests passed (`cargo test -p spacetop app::keys::tests`, 15 passed), but they do not cover fallback-created duplicate bindings.
- DONE: AC-4 session state is typed and stable per workflow. Evidence: `WorkflowSession`, `WorkflowScope`, and `WorkflowSessionKey` are typed in core; workflow keys canonicalize absolute workflow paths; app/session restore applies config defaults first and then saved session state. Targeted tests passed for canonical session save/restore and config-vs-session precedence.
- DONE: Proof commands:
  - `cargo test --workspace` -> passed: spacetop lib 311 passed; spacetop main 4 passed; spacetop integration tests 10/4/5 passed; spacetop-core lib 144 passed; core integration tests 7/1/2 passed; watcher real-backend tests 3 ignored; doctests 0.
  - `make lint` -> passed: `cargo clippy --all-targets --all-features -- -D warnings`.
  - `cargo test -p spacetop-core --test no_write_git_calls` -> passed, 2 passed.

### Verdict

REJECTED. The required proof commands pass, and AC-1/AC-2/AC-4 are supported by code and tests, but AC-3 is not complete because the final resolved keymap can contain duplicate bindings created by fallback behavior.

### Feedback Cycles

- Cycle 1: Verify rejected AC-3 because `ResolvedKeymap::from_config` checks duplicates before invalid/reserved bindings fall back to defaults. Configs such as `search: ""` plus `command: "/"`, or `search: "a"` plus `command: "/"`, can resolve both actions to `/`, making command unreachable with no final duplicate warning. Fix final keymap validation so fallback-created duplicates cannot survive, and add targeted tests for these cases.

## Cycle 1 Fix: implement

- DONE: Added final resolved-keymap duplicate validation after invalid, reserved, and pre-fallback duplicate decisions.
- DONE: Preserved accepted custom bindings when a fallback-created duplicate appears; fallback entries are reassigned to an unused canonical default.
- DONE: Surfaced `final duplicate keybinding` warnings when final collision repair is needed.
- DONE: Added regressions for `search: ""` plus `command: "/"` and `search: "a"` plus `command: "/"`; both assert distinct final bindings, preserved command `/`, final duplicate warnings, and reachable command action.

### Cycle 1 Proof Commands

- `cargo test -p spacetop fallback_does_not_collide` -> passed, 2 passed.
- `cargo test -p spacetop app::keys::tests` -> passed, 17 passed.
- `cargo test --workspace` -> passed: spacetop lib 313 passed; main 4 passed; integration tests 10/4/5 passed; spacetop-core lib 144 passed; core integration tests 7/1/2 passed; watcher real-backend tests 3 ignored; doctests 0.
- `make lint` -> passed, `cargo clippy --all-targets --all-features -- -D warnings`.
- `cargo test -p spacetop-core --test no_write_git_calls` -> passed, 2 passed.

## Stage Report: verify cycle 2

- DONE: AC-1 config and session paths are safe. Evidence: `config_path` and `state_path` only accept non-empty absolute `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, and `HOME` values, with relative XDG values falling back and relative HOME values yielding no path. Production startup loads config through `load_config_with_warnings(&StdEnv)` and uses `state_path(&StdEnv)` for session persistence; `save_session_file` rejects relative paths before creating directories or writing. Workspace tests include relative/empty XDG and relative HOME path cases, and the explicit no-write git guard still passes.
- DONE: AC-2 malformed config is visible but non-fatal. Evidence: `load_config_file_with_warnings` returns default config plus a `failed to parse config` warning on malformed YAML, `App` preserves config/keymap/runtime warnings, and the footer renders them through the shared warning surface. Workspace tests include `malformed_config_returns_default_with_warning` and `footer_surfaces_config_and_keymap_warnings`.
- DONE: AC-3 keybindings resolve through a validated final keymap. Evidence: `ResolvedKeymap::from_config` rejects invalid, duplicate, and reserved bindings before input handling, then runs `resolve_final_duplicates` over the final resolved bindings so fallback-created collisions cannot survive. The cycle-1 repros are covered by `invalid_search_fallback_does_not_collide_with_configured_command_slash` and `reserved_search_fallback_does_not_collide_with_configured_command_slash`; both assert distinct final labels, preserved configured command `/`, `final duplicate` warnings, and reachable command action. Help and footer render labels from `app.keymap()`.
- DONE: AC-4 session state is typed and stable per workflow. Evidence: `SessionState`, `WorkflowSession`, `WorkflowScope`, and `WorkflowSessionKey` are typed in core; workflow keys require absolute paths and canonicalize the workflow directory. App startup and lazy workflow materialization apply config defaults first, then saved session state, so persisted selected entity and scope override config defaults. Tests cover canonical session save/restore, saved selected entity restore, and session scope overriding config default scope.
- DONE: Proof commands:
  - `cargo test -p spacetop fallback_does_not_collide` -> passed, 2 passed.
  - `cargo test -p spacetop app::keys::tests` -> passed, 17 passed.
  - `cargo test --workspace` -> passed: spacetop lib 313 passed; main 4 passed; integration tests 10/4/5 passed; spacetop-core lib 144 passed; core integration tests 7/1/2 passed; watcher real-backend tests 3 ignored; doctests 0.
  - `make lint` -> passed: `cargo clippy --all-targets --all-features -- -D warnings`.
  - `cargo test -p spacetop-core --test no_write_git_calls` -> passed, 2 passed.

### Verdict

PASSED. The cycle-1 AC-3 defect is fixed: final keymap validation now repairs fallback-created duplicate bindings with warnings, and the required regression tests prove the formerly unreachable command binding remains reachable. AC-1, AC-2, and AC-4 are supported by code inspection and passing targeted/workspace tests.

## PR Review Fix: implement

- DONE: Addressed PR #54 color parsing feedback. `color_from_hex` now parses hex bytes instead of slicing string byte ranges, so non-ASCII invalid values return `None` instead of panicking.
- DONE: Addressed PR #54 session-key feedback. `WorkflowSessionKey::from_workflow_dir` now canonicalizes first, so existing relative workflow paths launched with `--workflow-dir` can restore/save session state under the canonical absolute key. Missing paths still fail to canonicalize.
- DONE: Addressed PR #54 startup config feedback. Startup config IO/read errors now fall back to default config plus a user-visible warning, preserving read-only workflow inspection.
- DONE: Updated the session-key regression that previously enforced relative workflow path rejection; existing relative paths now canonicalize. This supersedes older verify wording that workflow keys require absolute input paths.

### PR Review Proof Commands

- `cargo test -p spacetop ui::color::tests::non_ascii_hex_color_returns_none` -> red before fix with UTF-8 boundary panic; green after fix.
- `cargo test -p spacetop-core workflow_session_key` -> red before fix for existing relative path; green after fix, 3 passed.
- `cargo test -p spacetop startup_config_io_errors_fall_back_to_default_warning` -> green after adding startup fallback, 1 passed.
- `cargo test -p spacetop ui::color::tests` -> passed, 3 passed.
- `cargo test -p spacetop-core session_state::tests` -> passed, 10 passed.
- `cargo test --workspace` -> passed: spacetop lib 315 passed; main 4 passed; integration tests 10/4/5 passed; spacetop-core lib 145 passed; core integration tests 7/1/2 passed; watcher real-backend tests 3 ignored; doctests 0.
- `make lint` -> passed, `cargo clippy --all-targets --all-features -- -D warnings`.
- `cargo test -p spacetop-core --test no_write_git_calls` -> passed, 2 passed.
