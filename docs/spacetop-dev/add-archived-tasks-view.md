---
id: 006
title: Show completed (archived) tasks in the TUI
status: plan
source: captain feedback after build-initial-tui-overview
started: 2026-04-24T15:54:38Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

## Problem statement

The current TUI overview (built in task 003) loads only the active entities under a workflow directory — it deliberately skips `_archive/` and `_mods/` at the parser boundary (see `parser::load_workflow_dir` and the regression test `loads_workflow_snapshot_from_directory_ignoring_mods_and_archive`). That gives a clean "what is active now" view, but hides everything already shipped. Users have no in-TUI way to inspect what was completed, which verdict it received, when it finished, or what its stage reports said. Today the only way to answer "what did we ship?" is to `ls docs/<workflow>/_archive/` and open files in an editor.

This task makes archived entities first-class inspection targets in the read-only TUI, without disturbing the active workflow view that other stages rely on.

## Target user flow

A user runs `spacetop --workflow-dir docs/spacetop-dev` and sees the current active-task overview, exactly as today — nothing about the default startup view changes. The summary pane grows one line ("archived: N") so users can see at a glance that archived history is available.

Pressing `a` toggles the task list between two scopes:

- `active` (default, opens here): only entities whose files live at the workflow-dir root — today's behavior.
- `archived`: only entities under `_archive/` (including folder entities' `index.md`). The list shows the archived tasks, sorted by `completed` descending (fall back to filename when `completed` is missing).

The summary block labels the current scope ("view: active" / "view: archived") and the footer/help line shows `a: toggle archive view`. Navigation keys (`j`/`k`/Up/Down/Home/End), quit (`q`/`Esc`), and the preview pane behavior are unchanged across scopes — only the underlying item slice changes.

When the archived scope is active, preview content adds two fields next to the existing title/status/score/source/path/body excerpt:

- `verdict:` (e.g. `PASSED`, `REJECTED`, or `n/a`)
- `completed:` (the ISO timestamp, or `n/a`)

Archived rows in the list are visually muted (dim modifier) and append `[done ✓]` or `[done ✗]` style verdict badge at the end of the row so the archived scope is obviously not "work in progress." Active rows look exactly as today.

### UX justification

Options considered:

1. **Unified list with a filter cycling through active/archived/all** — simplest model but mixes two semantically different things (workflow you can act on vs. history). It also forces the default view to decide whether to show archives, and archived entities break the "all items have an allowed-status" invariant the parser currently enforces.
2. **Separate screen with its own keybinding** (chosen in concept, implemented as a scope toggle over the same list widget). Keeps the active overview untouched as the default, makes archive access opt-in per session, and doesn't require a new "all" slice or new sort rules for the active view.
3. **A tab bar / pane split** — more discoverable but over-engineered for v1 when we only have two scopes.

Option 2 wins because it keeps the existing active-task view as the first thing users see (backward-compatible with the TUI stage reports/tests), is trivially discoverable via the footer, and scales naturally if we later add `_mods/` or per-stage filters as additional scopes.

## Acceptance criteria

**AC-1 -- Archived entities are loadable via the parser without breaking the active-task snapshot.**
Verified by: a parser test asserts `load_workflow_dir` still returns only active items (no `_archive/` paths) and a new loader returns the archived entities from `_archive/*.md` and `_archive/*/index.md` fixtures. Active-snapshot regression test `loads_workflow_snapshot_from_directory_ignoring_mods_and_archive` still passes unchanged.

**AC-2 -- The TUI opens on the active-task view by default and reveals archived tasks only on explicit toggle.**
Verified by: an app-state test asserts a freshly loaded `App` has `view_scope == Active`, the summary pane renders `view: active`, and the rendered task list contains no archived titles. A second assertion after `handle_key('a')` shows `view_scope == Archived` and the list contains archived titles.

**AC-3 -- The archived view renders verdict and completion timestamp in the preview pane.**
Verified by: a render test against a fixture archived entity (e.g. a copy of `_archive/scaffold-rust-cli-project.md`) asserts the preview buffer contains `verdict: PASSED` and `completed: 2026-04-24T14:49:53Z` alongside the existing title/status/score/source/path fields.

**AC-4 -- The archive list is ordered by completion time (newest first) and counts appear in the summary.**
Verified by: an app-state test builds a snapshot with three archived items having distinct `completed` timestamps and asserts the archived ordering is newest-first, missing-timestamp items sort last, and the summary pane renders an `archived: 3` line.

**AC-5 -- Browsing the archived view does not mutate workflow files and navigation/quit keys behave the same in both scopes.**
Verified by: navigation tests cover `j`/`k`/Home/End/`q`/`Esc` in both scopes; a smoke run of `cargo run -- --workflow-dir docs/spacetop-dev` followed by `a`/`j`/`q` leaves `git diff -- docs/spacetop-dev` empty.

## Parser / TUI constraints

These pin down how archived state enters the data model and how it flows through the UI.

### Parser

- Active-snapshot parsing stays exactly as today — `load_workflow_dir` continues to ignore `_archive/` and `_mods/`, and `WorkflowSnapshot.items` remains "active items only." Downstream stage counts and the default view must not change.
- Add a sibling loader (e.g. `load_archived_items(&Path, &[stage_name]) -> Result<Vec<WorkItem>, ParseError>`) that reads markdown from `_archive/*.md` and `_archive/*/index.md`. It reuses `parse_work_item` and the existing `ParseError` variants.
- `WorkItem` gains no new fields — `completed` and `verdict` already exist as `Option<String>`. Archived items populate them from frontmatter; missing values render as `n/a`.
- Allowed-status validation: archived entities typically carry `status: done` (the terminal stage), so the existing `allowed_statuses` check passes naturally. The loader uses the same allow-list derived from the workflow README stages — no bypass, no new error kind.
- Loading is **opt-in / on-demand**, not default-on. The app calls the archive loader only when the user first toggles to the archived scope (or at startup alongside active load — either is acceptable, plan stage picks one). This keeps default TUI startup cost identical and avoids parsing historical files when the user never looks at them.
- Archive loader errors are isolated from active load: a malformed file under `_archive/` must not prevent the active view from rendering. Surface the error in a status line within the archived scope instead of aborting app startup.

### Domain / App state

- Introduce a `ViewScope { Active, Archived }` enum on `App` with `Active` as the default. `selected_index` is per-scope (either separate fields, or reset on toggle — plan stage decides).
- Add either a second `Vec<WorkItem>` for archived items on `App` (parallel to `snapshot.items`) or wrap both slices behind a method like `visible_items(&self) -> &[WorkItem]`. Existing callers (`selected_item`, `stage_counts`, render code) should pick their slice through the scope, not by mutating `snapshot.items`.
- `stage_counts` keeps reading from the active snapshot only. The archived summary line is a separate count derived from the archived slice.

### TUI

- The summary block gains one scope indicator line and one archived-count line. It must still render correctly when the archived slice has not been loaded yet (show `archived: -` or `archived: (press a)` — plan stage picks exact copy).
- The task list widget is reused; only its input slice changes with scope. Selected-row styling stays the same. Archived rows render with `Modifier::DIM` and a `[✓]`/`[✗]` verdict glyph appended after the title (fallback to `[?]` when verdict is missing).
- The preview widget branches on scope: active shows today's fields; archived additionally renders `verdict:` and `completed:` lines between `source:` and `path:`.
- Keybindings: `a` toggles scope. All existing keys keep their current semantics. Help text in the summary footer lists `a` alongside navigation hints.
- Read-only contract holds — no code path added in this task may write to the workflow directory. Smoke run must leave `git diff` empty.

### Explicitly out of scope

- No new `status: archived` value — archival is a file-location fact, not a stage.
- No "all" combined scope. Users who want both views toggle between the two.
- No edit/restore affordances. The TUI remains read-only.
- No change to the `--workflow-dir` CLI surface. The archived scope uses the same root.

## Stage Report: design

- DONE: Problem statement and user flow are written for how archived tasks integrate with the current TUI (unified list vs. separate view, toggle key, default state).
  Wrote problem/flow sections above — chose scope-toggle over a unified/filter list, key binding `a`, default scope `Active`, archived rows muted with verdict badge.
- DONE: Acceptance criteria in the entity replace the placeholder section with concrete, verifiable AC-N bullets covering archived browsing, preview fields, and default-view behavior.
  Replaced the placeholder ACs with AC-1…AC-5 covering parser loader, default-view, preview verdict/completed fields, archive sort + count, and read-only navigation parity.
- DONE: Parser/TUI constraints are named — specifically how `WorkflowSnapshot` loads `_archive/` entries (default on, opt-in flag, or lazy) and any impact on existing active-task rendering.
  Added Parser/TUI constraint section — `WorkflowSnapshot` stays active-only, a sibling `load_archived_items` loader is opt-in/on-demand, `WorkItem` unchanged, `App` gets a `ViewScope`, preview branches on scope, summary gains a scope+count line.

### Summary

Locked the archive view as an opt-in scope toggle (`a` key) layered over the existing active-task overview rather than a unified filter list or a new screen — this preserves today's default UX and keeps the parser's active-snapshot contract untouched. Parser work adds a sibling archive loader that reuses `parse_work_item` and existing `WorkItem` fields (`completed`, `verdict`), so domain types do not change. ACs now pin backward compatibility, preview verdict/completed fields, sort-by-completed order, and read-only smoke evidence, which gives the plan stage unambiguous targets.
