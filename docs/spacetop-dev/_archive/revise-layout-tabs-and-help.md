---
id: 011
title: Revise dashboard layout (responsive width, pane-centered content), tab bar for workflow switcher, surface help affordance
status: done
source: captain feedback after 010 ship
started: 2026-04-25T06:18:08Z
completed: 2026-04-25T06:29:23Z
verdict: PASSED
score:
worktree: 
issue:
pr:
mod-block: 
archived: 2026-04-25T06:29:23Z
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

## Stage Report: implement

- DONE: Dashboard fills terminal width, pane content is centered, tab bar with `←`/`→` ships, help popup affordance is visible — each verified by at least one render or app-state test.
  Render tests `dashboard_pane_spans_full_terminal_width`, `graph_ribbon_node_row_is_horizontally_centered_in_pane`, `multi_session_renders_tab_bar_with_count_and_per_workflow_tabs`, `arrow_keys_cycle_active_tab_with_wraparound_in_multi`, `arrow_keys_inert_in_single_session`, `single_session_omits_tab_bar`, `dashboard_status_footer_lists_help_affordance`, `help_popup_includes_arrow_keys_in_multi_session` cover each requirement.
- DONE: The two overrides (task 009 centered-column and task 010 tabs-rejected) are explicitly named in the stage report along with what code was retired or rewritten.
  Task 009 override: `MAX_CONTENT_WIDTH=120` cap and the `centered_column` width-cap in `src/ui/mod.rs` were dropped; the dashboard now fills `frame.area()` directly. Pane-internal content centering is achieved via `center_horizontal` for task list / preview and `Alignment::Center` on the graph ribbon Paragraph. Task 010 override: the `]`/`[` cycle bindings were removed in favor of `Left`/`Right`; `OverviewSession::breadcrumb_label` was deleted; `render_stage_graph_with_breadcrumb` was collapsed back into `render_stage_graph` with the `[i/N]` title prefix retired; the new `render_tab_bar` (`src/ui/mod.rs`) carries the workflow-count and per-workflow tabs. `P` picker-overlay is preserved; `?` was already wired in `App::handle_key` for Overview mode (no bug — captain's complaint was discoverability, fixed via the new `render_status_footer`).
- DONE: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all fully clean on the worktree branch — no surviving "pre-existing on main" carve-outs.
  `cargo fmt --check` clean. `cargo clippy --all-targets -- -D warnings` clean. `cargo test` 103/103 passed across the four binaries (lib 99, integration suites 0/4/0/0).

### Summary

Replaced the wide-terminal centered-column rule with full-width dashboard layout while centering content blocks inside each pane. Added a real tab bar above the graph ribbon for multi-workflow sessions (one tab per workflow, `(active/total)` count visible, `Left`/`Right` cycle with wrap-around) and retired the `]`/`[` keybindings and the graph-title `[i/N]` breadcrumb. Surfaced the help affordance via a centered status-line footer at the bottom of the Overview and added the `Left`/`Right` switch entries to the help popup body. No app-state surface beyond key bindings and the removal of `breadcrumb_label` was changed.

## Stage Report: review

- DONE: AC-1 — dashboard fills 200×40 wide TestBackend and pane content is centered.
  `dashboard_pane_spans_full_terminal_width` (top border at col 0 and col 199 both non-blank) plus `graph_ribbon_node_row_is_horizontally_centered_in_pane` pass; graph ribbon uses `Alignment::Center`, task list and preview use `center_horizontal(inner, PANE_CONTENT_TARGET=70)` in `src/ui/mod.rs:283-289` and `:347-354`.
- DONE: AC-2 — multi-workflow tab bar with `(active/total)` count, `Left`/`Right` cycle with wrap-around, no double affordance.
  `multi_session_renders_tab_bar_with_count_and_per_workflow_tabs` asserts `Workflows (1/3)` plus 3 tabs; `arrow_keys_cycle_active_tab_with_wraparound_in_multi` walks 0→1→2→0 (wrap) and Left wrap 0→2; `grep "KeyCode::Char(']')\|KeyCode::Char('[')" src tests` returns nothing — old bindings retired.
- DONE: AC-3 — single/`-w` sessions hide the tab strip and ignore arrows.
  `single_session_omits_tab_bar` asserts `Workflows (` absent; `arrow_keys_inert_in_single_session` confirms no `pending_switch` and active_index unchanged. The `is_multi`-gated branches in `render_overview` (mod.rs:120-149) and `App::handle_key` (app.rs:888-895) implement zero-row cost.
- DONE: AC-4 — `?` opens the popup from Overview and the popup lists Left/Right keys.
  `help_popup_includes_arrow_keys_in_multi_session` asserts both `→`/`Right` and `←`/`Left` plus `re-discover` in multi, and absence of cycle hints in single. `?` handler is wired in all three modes (`app.rs:881`, `:903`, `:938`).
- DONE: AC-5 — visible status-line affordance with `?` + `help` substrings.
  `dashboard_status_footer_lists_help_affordance` asserts `?`, `help`, and `q: quit`. `render_status_footer` (mod.rs:204-219) emits `?: help   ←/→: switch workflow   P: pick   a: archive   q: quit` (multi) or `?: help   a: archive   q: quit` (single).
- DONE: AC-6 — `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` clean.
  Re-ran in worktree: fmt silent, clippy `Finished` with no warnings, tests 99 lib + 4 discovery integration passed (1 watcher_fs ignored by design — real notify backend).
- DONE: Two overrides cleanly retired.
  `grep -rn "MAX_CONTENT_WIDTH\|centered_column\|render_stage_graph_with_breadcrumb\|breadcrumb_label\|KeyCode::Char(']')\|KeyCode::Char('[')" src tests` returns zero matches; orphan tests `breadcrumb_appears_in_header_when_multi` and `no_breadcrumb_in_single_workflow` were rewritten as `no_breadcrumb_in_graph_header`.
- DONE: Diff scope contained.
  `git diff --name-only main...HEAD` lists only `docs/.../revise-layout-tabs-and-help.md`, `src/app.rs`, `src/lib.rs`, `src/ui/graph.rs`, `src/ui/mod.rs` — no parser/discovery/watcher drive-by.

### Summary

Verdict: PASSED. Both overrides (task 009 width-cap, task 010 breadcrumb-tabs swap) are cleanly retired with grep-verified deletions and rewritten tests; new tab bar, status footer, and `Left`/`Right` keymap behave per ACs across multi and single sessions; fmt/clippy/test all clean on the worktree. No defects found.
