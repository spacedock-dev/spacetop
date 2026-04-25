---
id: "015"
title: Fix tig-style preview UX, workflow switching edge cases, and loader/parser failures
status: done
source: captain feedback 2026-04-25
started: 2026-04-25T12:50:00Z
completed: 2026-04-25T14:37:52Z
verdict: PASSED
score: 0.9
worktree: 
issue:
pr: "#5"
mod-block: 
archived: 2026-04-25T14:37:52Z
---

Captain drove a concentrated polish pass across the overview dashboard. The work started as a `tig`-style preview UX request, then expanded into a full round of interaction fixes, layout corrections, parser hardening, and workflow switching/debugging fixes found while using the app against real Spacedock repositories.

The goal of this task was to make the dashboard behave predictably in daily use: list-first by default, preview on demand, adaptive preview placement, clearer help text, stable archive/workflow switching, resilient workflow discovery, and graceful handling of imperfect markdown frontmatter in real repos.

## Scope

### 1. Preview-mode UX and adaptive layout

- Start in list mode with preview hidden.
- `Enter` toggles preview mode on and off.
- In preview mode, `Up` and `Down` continue switching tasks while the preview content updates.
- Place the preview on the right only when the overall dashboard is wider than twice its height; otherwise stack it on the bottom.
- When stacked on the bottom, split the available area 30% task list and 70% preview.
- Support horizontal preview scrolling with `Left` and `Right`.
- Keep preview metadata readable:
  - `status: ● done` formatting
  - compact `status / score / source / verdict` onto one line in bottom-preview mode
  - keep the `── body ...` divider visible even when long headers wrap

### 2. Key handling and help affordances

- In list mode, `Left` and `Right` switch workflows; in preview mode, the same keys scroll preview content.
- `PgUp` and `PgDn` page the task or archived list when preview is closed, and continue to scroll preview when preview is open.
- `q` closes preview mode before quitting the app.
- `q` also closes the workflow picker popup the same way as `Esc`, instead of quitting the process.
- Update footer/help wording to say `pick workflow`, and hide that hint when only one workflow exists.

### 3. Workflow picker, archive view, and graph polish

- Make the workflow picker a bordered popup with minimal height.
- Fix archived-view reload so switching workflows away and back does not leave archived mode selected with an empty list.
- Remove duplicate workflow-path rendering from the graph ribbon when the header bar already shows the path.
- Expand the workflow stage color pool so common 5-stage workflows use a richer set of colors instead of alternating between two.

### 4. Parser and workflow-loading resilience

- Allow `-w/--workflow-dir` pointing at a repo root to fall back to workflow discovery instead of failing on a non-workflow `README.md`.
- Add a flat-frontmatter fallback so unquoted colons in simple YAML values do not abort work-item loading.
- Skip malformed archived entries instead of failing the entire archived view.

## Acceptance criteria

**AC-1 -- Overview behaves as a list-first dashboard with a `tig`-style preview mode.**
Verified by: app-state and render tests covering hidden-by-default preview, `Enter` toggling, preview-mode navigation, adaptive placement, bottom split, horizontal scroll, and wrapped-header divider visibility.

**AC-2 -- Overview key help and key behavior match the current mode.**
Verified by: tests covering preview-vs-list `Left`/`Right` semantics, page key behavior, `q` closing preview/picker overlays first, and footer/help strings that change with preview state and workflow count.

**AC-3 -- Workflow switching and archived mode remain stable across real navigation paths.**
Verified by: tests for per-workflow state preservation, archived reload correctness, picker behavior, and graph/header rendering without duplicate path context.

**AC-4 -- Real-world workflow inputs fail gracefully instead of crashing the app.**
Verified by: parser/discovery tests covering repo-root fallback, flat-frontmatter parsing with unquoted colons, and skipping malformed `_archive` entries while preserving valid ones.

**AC-5 -- Full regression suite remains clean after the combined fix pass.**
Verified by: `cargo test`.

## Stage Report: implement

- DONE: Reworked overview state and key handling so preview is off by default, `Enter` toggles preview mode, preview mode keeps list navigation active, `Left`/`Right` scroll preview only while preview is open, and `PgUp`/`PgDn` page the task/archive list when preview is closed.
  Implemented in `src/app.rs` and `src/ui/mod.rs`, including the visible-list page-size tracking used by list paging.
- DONE: Rebuilt preview placement and rendering behavior to match the final UX rules from captain feedback.
  The placement rule now uses the overall dashboard dimensions, opens side-by-side only when `width > height * 2`, renders side preview on the right, stacks bottom preview at 30/70, supports horizontal scrolling, preserves the body divider under wrapped headers, and formats preview metadata differently for bottom-vs-side layouts.
- DONE: Tightened quit/help/picker behavior to match expected TUI overlay semantics.
  `q` closes preview first, `q` closes the picker popup like `Esc`, footer/help copy now says `pick workflow`, the hint is hidden for single-workflow sessions, and `Left`/`Right` help text changes between `switch workflow` and `preview scroll` based on mode.
- DONE: Fixed workflow/archived edge cases and visual duplication.
  Archived state is reloaded when switching workflows while archived mode is active, the graph ribbon no longer repeats the workflow path already shown in the header, and the workflow picker now renders as a bordered minimal-height popup.
- DONE: Expanded stage-color assignment and graph coloring coverage.
  Unknown workflows now pull from a larger color pool so common five-stage flows render with distinct colors instead of collapsing into two-color alternation.
- DONE: Hardened workflow loading and parsing against real repository inputs.
  `--workflow-dir` now falls back to discovery when pointed at a repo root, flat frontmatter with unquoted colons can still parse, and malformed archived files are skipped so archived view stays usable.
- DONE: Added and updated regression coverage across overview behavior, parser fallbacks, discovery fallback, archive loading, and render/layout behavior.
  The combined session raised the test count while preserving clean full-suite execution.

### Summary

This implementation pass turned the dashboard into a usable `tig`-style inspector instead of a permanently split overview, then followed through on every adjacent paper cut exposed during real usage: preview placement math, quit semantics, help wording, archive reloads, duplicate path display, stage coloring, repo-root loading, tolerant frontmatter parsing, malformed archive handling, and list paging.

## Stage Report: review

- DONE: AC-1 satisfied by named overview and render tests for preview toggling, placement, sizing, metadata layout, horizontal scroll, and header-wrap resilience.
  Evidence includes `overview_hides_preview_until_enter_opens_preview_mode`, `preview_opens_on_right_in_wide_terminals_and_bottom_in_taller_ones`, `bottom_preview_compacts_metadata_into_one_line`, `preview_right_key_horizontally_scrolls_long_lines`, and `preview_keeps_body_divider_visible_when_header_wraps`.
- DONE: AC-2 satisfied by key-behavior and help-surface tests.
  Evidence includes `preview_mode_consumes_left_right_for_horizontal_scroll_in_multi`, `multi_footer_shows_switch_workflow_when_preview_closed`, `multi_footer_shows_preview_scroll_when_preview_open`, `q_closes_preview_before_quitting_overview`, `picker_overlay_q_closes_popup_without_quitting`, and `page_keys_move_task_selection_when_preview_is_closed`.
- DONE: AC-3 satisfied by workflow/archive stability and graph-header tests.
  Evidence includes `switch_preserves_per_workflow_state`, archived-view regression coverage in `src/app.rs`, `renders_bordered_dialog` in `src/ui/picker.rs`, and graph-header assertions that keep only one workflow path on screen.
- DONE: AC-4 satisfied by parser/discovery regression tests.
  Evidence includes `explicit_w_repo_root_falls_back_to_discovery_within_that_root`, `explicit_w_repo_root_single_workflow_opens_that_workflow`, `parses_flat_frontmatter_with_unquoted_colon_in_title`, and malformed-archive tests that preserve valid archived entries.
- DONE: AC-5 satisfied by a clean full test suite after the combined patch set.
  `cargo test` passed with 135 unit tests, 7 integration tests, 1 ignored real-backend watcher test, and 0 failures.

### Summary

Verdict: PASSED. The dashboard now matches the requested interaction model and the follow-on fixes remove several real-world failure modes discovered during the same session. The diff is broad but coherent: it stays inside overview state/rendering, parser/loading, and related regression tests, with `cargo test` clean at the end.
