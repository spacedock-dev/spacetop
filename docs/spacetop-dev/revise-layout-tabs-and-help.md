---
id: 011
title: Revise dashboard layout (responsive width, pane-centered content), tab bar for workflow switcher, surface help affordance
status: implement
source: captain feedback after 010 ship
started: 2026-04-25T06:18:08Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-revise-layout-tabs-and-help
issue:
pr:
mod-block: 
---

Combined UI/UX revision task. Captain asked to skip design/plan and implement directly.

This task **overrides two earlier locked design decisions** — call them out explicitly in the implement stage report:

- Task 009 centered the whole dashboard column when the terminal was wider than ~120 cols. Captain now wants the dashboard to use the full screen width; instead, content *inside* each pane should be centered.
- Task 010 rejected tabs in favor of a status-line breadcrumb (`[i/N] path`, `]`/`[` to cycle, `P` for picker-overlay). Captain now wants a real tab bar across the top of the dashboard with `←`/`→` to switch.

## Scope

### 1. Responsive dashboard sizing + content-centered panes

- Drop the ~120-col centered-dashboard cap that task 009 introduced. The dashboard should fill the available terminal width (and adapt as it resizes).
- Within each pane (graph ribbon, task list, preview), horizontally center the *content* so it doesn't hug the left edge on wide terminals.
  - Graph ribbon: center the node row, counts row, and feedback-arc row inside the pane width.
  - Task list: center the column of task rows inside the list pane (rows themselves can stay left-aligned within their fixed visible-column block — it's the column-block that gets centered).
  - Preview: center the preview content block inside the preview pane.
- The picker screen + help popup may keep their existing centering — they are overlays, not the dashboard itself.

### 2. Tab bar workflow switcher

- Add a tab strip at the top of the overview screen (above the graph ribbon). One tab per discovered workflow. The active tab is highlighted; other tabs are dimmed.
- `←` (Left) cycles to the previous tab, `→` (Right) cycles to the next. Wrap-around at the ends.
- Show the workflow count somewhere visible (e.g. tab strip suffix `(2/5)` or in the help popup) — captain explicitly wants the user to "see how many workflows in this repo in the main dashboard".
- Single-workflow / `-w`-pinned sessions: hide the tab strip entirely (no row cost). Cycle keys do nothing in that mode.
- Free up the `]`/`[` keybinds from task 010 (move their behavior onto `←`/`→`, or drop them entirely).
- Keep the `P` picker-overlay for re-discovery — captain didn't ask to remove it, and it's the affordance to add a newly-commissioned workflow without restart.
- Keep the existing `OverviewSession` / `Vec<OverviewState>` per-workflow state preservation, watcher restart, and pinned-single semantics — only the *UI surface* changes.

### 3. Help popup affordance

- Captain reports "no help popup widget in the main dashboard". Two possible interpretations:
  - **Discoverability gap**: the `?` binding exists (task 009 added it) but the user can't find it. Add a visible hint — e.g. a status-line footer `?: help  ←/→: switch  q: quit` — so the affordance is on screen.
  - **Actual bug**: the `?` binding doesn't toggle in Overview mode for some reason (maybe gated by a stale AppMode check after task 010's PickerOverlay variant).
- Investigate first, then fix whichever is the real issue. Both fixes are in scope if both apply.
- The help popup itself, when triggered, should list the new keymap (including `←`/`→` for tabs).

## Acceptance criteria

**AC-1 -- The dashboard fills the terminal width; content inside each pane is horizontally centered.**
Verified by: render test against a wide TestBackend (e.g. 200×40) asserts (a) the dashboard pane block spans the full width with negligible outer margin and (b) within the graph ribbon pane, the node row's leftmost glyph is roughly equidistant from the pane left/right edges.

**AC-2 -- A tab bar across the top shows one tab per discovered workflow when 2+ workflows exist; `←`/`→` cycle the active tab; count of total workflows is visible in the dashboard.**
Verified by: render test on a multi-workflow fixture asserts the tab strip is present and shows `N` tabs; key handler test asserts `←`/`→` rotate the active index with wrap-around.

**AC-3 -- Single-workflow and `-w/--workflow-dir`-pinned sessions hide the tab strip and ignore `←`/`→` (zero UI cost).**
Verified by: render test on a single-workflow fixture asserts the tab strip block is absent / zero-height; key handler test asserts `←`/`→` are no-ops when only one workflow is loaded.

**AC-4 -- The `?` help popup is reachable from the Overview mode and the keymap inside it includes the new tab switcher keys.**
Verified by: integration test or app-state test asserts pressing `?` from Overview opens the popup with non-empty content; grep / render assertion finds `←` and `→` (or `Left`/`Right`) listed in the popup body.

**AC-5 -- A visible affordance for the help popup exists in the dashboard (e.g. status-line hint).**
Verified by: render test asserts a literal substring like `?` and `help` appear in the dashboard footer / status row in Overview mode.

**AC-6 -- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` are all fully clean on the worktree branch.**
Verified by: command output cited in the implement stage report.
