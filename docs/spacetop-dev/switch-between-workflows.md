---
id: 010
title: Switch between multiple discovered workflows from inside the TUI
status: design
source: captain feedback after 009 ship
started: 2026-04-25T04:13:38Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

When a repo contains multiple Spacedock workflows (e.g. `docs/spacetop-dev/`, plus future product or research workflows), the TUI should let the user move between them without quitting and relaunching. The autodiscovery picker (task 005) handles the initial selection at startup; this task adds the in-session switch.

Captain's working hypothesis: a tab strip across the top, one tab per discovered workflow. Design should validate or reject that — alternatives include a workflow chooser overlay (a sibling to the help popup), a persistent picker key (re-open the existing picker mid-session), or a status-line breadcrumb with hotkey navigation.

Open design questions to resolve in the `design` stage:

- Multi-tab UI vs. modal chooser vs. keybinding-only switch — which best fits the current dashboard density (graph ribbon + task list + preview, plus task 009's centered column and help popup)?
- Where does the active-tab indicator live? On top of the ribbon? Under the title row? Replacing part of the header?
- Keybindings: `Tab` / `Shift+Tab` to cycle, numeric (`1`..`9`) for direct selection, or both? Conflicts with existing keys (`a`, `?`, `j`/`k`, picker keys)?
- Per-tab state: does each workflow keep its own selection, scope (`Active`/`Archived`), and watcher (task 008), or do we tear down on switch and reload? Memory cost vs. switch latency.
- Discovery refresh: when a tab is created, deleted, or renamed on disk between sessions, how does the tab strip notice? (Task 008's watcher only watches the *current* workflow dir.)
- Single-workflow case: tab strip hidden entirely, or shown with one tab? (Pick a default that doesn't waste a row.)
- Picker entry point in the multi-tab model: does the startup picker still exist, or does the TUI always open with all discovered tabs? If the user passed `-w/--workflow-dir`, that becomes a single-tab session — should `+` open the discovery list to add another tab, or is single-tab a strict mode?
- Re-discovery: after the TUI is open, does the user have a key to re-scan for newly added workflows (e.g. the user just commissioned one in another shell)?
- Watcher fan-out: do we run one watcher per tab (parallel `notify` watchers, more file handles) or only watch the active tab and rescan on tab focus?

## Acceptance criteria

_To be firmed up during design. Expected shape:_

**AC-1 -- When the repo contains 2+ workflows, the user can switch between them inside the TUI without restarting.**
Verified by: integration test against a fixture repo with two workflow dirs; assert the visible workflow changes after the switch keybinding.

**AC-2 -- Single-workflow repos do not pay a UI cost (no empty tab row, no extra keybind noise).**
Verified by: render test on a single-workflow fixture asserts the chosen UI element is hidden.

**AC-3 -- Switching workflows preserves per-workflow state to a documented degree (selection, scope) so the user can flip back without losing context.**
Verified by: app-state test simulates select → switch → switch back, asserts selection/scope intact.

**AC-4 -- Tab/chooser keybindings do not collide with existing bindings (`a`, `?`, `j`/`k`, `↑`/`↓`, `Home`/`End`, `Enter`, `q`, `Esc`).**
Verified by: keymap audit in the help popup and a unit test enumerating bindings.

**AC-5 -- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` are all clean on the implement branch.**
Verified by: command output cited in the implement stage report.
