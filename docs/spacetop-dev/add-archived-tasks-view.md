---
id: 006
title: Show completed (archived) tasks in the TUI
status: review
source: captain feedback after build-initial-tui-overview
started: 2026-04-24T15:54:38Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-add-archived-tasks-view
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

## Implementation plan

The plan splits into four layers so the implement stage can land them as roughly-sequential commits, each independently verifiable with `cargo test`. No `src/` writes happen in this plan stage.

### Layer 1 — Parser: archive loader (pure logic, no TUI)

Files:
- `src/parser.rs` — add a sibling loader alongside `load_workflow_dir`.
- `src/domain/mod.rs` — no field additions. Add one small helper type if needed (see below).
- `docs/spacetop-dev/_archive/*.md` — fixtures already exist (`scaffold-rust-cli-project.md`, `parse-spacedock-workflow-files.md`, `build-initial-tui-overview.md`). Use them as read-only fixtures.

New parser API (in `src/parser.rs`):
- `pub fn load_archived_items(workflow_dir: &Path, allowed_statuses: &[String]) -> Result<Vec<WorkItem>, ParseError>`
  - Reads `workflow_dir/_archive/`. Missing `_archive/` is treated as `Ok(Vec::new())` (not an error) — `_archive` is optional.
  - For each direct child of `_archive/`:
    - If it is a `*.md` file, parse via `parse_work_item`.
    - If it is a directory, parse `<dir>/index.md` via `parse_work_item` (skip silently if `index.md` is missing — folder entities may legitimately be bare).
  - Ignore nested `_archive/*/*.md` other than `index.md`.
  - Sort results newest-first by `completed` (string compare on the ISO-8601 timestamp — lexicographic ordering matches chronological for this format). Items with `completed == None` sort last; within that group, fall back to filename ascending so ordering is deterministic.
  - Reuses `parse_work_item` and therefore the existing `allowed_statuses` validation. No new `ParseError` variant.
- Keep `load_workflow_dir` untouched. Regression test `loads_workflow_snapshot_from_directory_ignoring_mods_and_archive` must keep passing without edits.

Optional domain helper (only if ergonomic):
- Add `pub fn archive_dir(workflow_dir: &Path) -> PathBuf { workflow_dir.join("_archive") }` near the other path helpers, to keep the `_archive` literal in one place. Skip if it creates unnecessary churn.

Verification:
- `cargo fmt --all`
- `cargo test -p spacetop parser::tests` (or the bin-equivalent — project is a single crate; plain `cargo test` is fine).

### Layer 2 — App state: `ViewScope` and archive slice

Files:
- `src/app.rs` — add scope enum, archive slice, per-scope selection, toggle handler.
- `src/domain/mod.rs` — no changes expected.

Additions in `src/app.rs`:
- `pub enum ViewScope { Active, Archived }` with `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` and `Default = Active` (either `#[derive(Default)]` with `#[default]` on `Active`, or an explicit `Default` impl).
- Extend `App` with:
  - `view_scope: ViewScope` (default `Active`).
  - `archived_items: Vec<WorkItem>` (empty on construction; populated once on demand).
  - `archive_loaded: bool` — gates the one-time load.
  - `archive_error: Option<String>` — displayed in the archived scope's list area if loading failed; active view never sees this.
  - `selected_index_archived: usize` — separate selection state so toggling scope restores the user's place.
- Rename internal `selected_index` to `selected_index_active` (or keep `selected_index` as the active cursor and add `selected_index_archived` — implement stage picks whichever reads cleaner). The public accessor `selected_index()` returns the one for the current scope.
- New methods:
  - `pub fn view_scope(&self) -> ViewScope`
  - `pub fn visible_items(&self) -> &[WorkItem]` — returns either `&self.snapshot.items` or `&self.archived_items` based on scope.
  - `pub fn archived_items(&self) -> &[WorkItem]`
  - `pub fn archived_count(&self) -> Option<usize>` — returns `None` when `!archive_loaded` (UI renders `archived: (press a)`), `Some(n)` after load.
  - `pub fn archive_error(&self) -> Option<&str>`
  - `fn ensure_archive_loaded(&mut self)` — calls `load_archived_items(&self.workflow_dir, &allowed)` the first time it is invoked; sets `archive_loaded = true` regardless of outcome so we do not retry on every toggle. Errors go into `archive_error`, not a panic.
- `selected_item()` now reads from `visible_items()` using the per-scope index.
- `stage_counts()` stays wired to `self.snapshot.items` only (active counts; archived count is shown separately).
- `handle_key`:
  - Add `KeyCode::Char('a') => self.toggle_scope()`. `toggle_scope` flips `view_scope` and calls `ensure_archive_loaded()` on the first transition to `Archived`. It must clamp the per-scope selection if the newly-visible list is empty.
  - All other keys (`j`/`k`/Up/Down/Home/End/`q`/`Esc`) keep their semantics but operate on the current scope's selection + visible slice (via `visible_items().len()`).

Verification:
- `cargo test app::tests` — all existing tests pass without semantic changes; the `selected_index`/`snapshot` accessors still return active-scope data by default.

### Layer 3 — UI: summary, list badges, preview fields, help footer

Files:
- `src/ui/mod.rs` — update `summary`, `task_list`, `preview` functions and their tests.

Summary block changes:
- Append two lines below the existing `workflow: ...` line: `view: active` | `view: archived` and `archived: N` | `archived: (press a)` (before the archive has been loaded) | `archived: load failed` (when `archive_error` is set).
- Append a footer hint line at the bottom of the summary block: `keys: j/k nav, a toggle archive, q quit`.
- The summary block height constraint (`Constraint::Length(10)`) may need to grow (e.g., `Length(12)`). Revisit in implement stage; acceptable to pick a slightly larger length to fit the new lines.

Task list changes (`task_list`):
- Source items from `app.visible_items()` (not `app.snapshot().items`).
- When `app.view_scope() == ViewScope::Archived`:
  - Append a verdict glyph to each row: `[✓]` if `verdict == Some("PASSED")`, `[✗]` if `verdict == Some("REJECTED")` (or any non-PASSED value), `[?]` if `verdict.is_none()`.
  - Style non-selected rows with `Modifier::DIM` so archived rows look visually muted.
  - Selected row keeps `Modifier::REVERSED` (takes precedence over DIM) so the cursor remains visible.
- When the archived scope is empty but `archive_error` is `Some`, render the error string as a single line in place of items (`Line::from(Span::styled(...))`).

Preview changes (`preview`):
- When `app.view_scope() == ViewScope::Archived`, insert two extra `Line::from(...)` entries between the existing `source:` and `path:` lines:
  - `verdict: {item.verdict.as_deref().unwrap_or("n/a")}`
  - `completed: {item.completed.as_deref().unwrap_or("n/a")}`
- Active scope preview is unchanged.

Verification:
- `cargo test ui::tests` — existing render tests pass; new render tests (Layer 4) cover the archived-scope branches.

### Layer 4 — Terminal wiring and smoke

Files:
- `src/lib.rs` — **no structural change required.** `App::load` continues to load only the active snapshot; the archive loader is invoked inside `App::toggle_scope`, not at startup. The event loop already delegates every key to `app.handle_key`, so `a` is handled transparently.
- Double-check that no new `stdout`/`stderr` writes are introduced in the archive path (read-only contract).

Verification:
- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings` (if the project already uses clippy CI; otherwise at least `cargo check --all-targets`).
- `cargo test` — full suite.
- Manual smoke (AC-5): `cargo run -- --workflow-dir docs/spacetop-dev` → press `a` (should flip to archived, listing the three archive entries newest-first), press `j`/`k` to navigate, `a` again to flip back, `q` to quit. Then `git diff -- docs/spacetop-dev` must be empty.

## Focused test strategy

Tests are grouped by module and named precisely so the implement stage has a drop-in list.

### Parser tests (`src/parser.rs`, `#[cfg(test)] mod tests`)

1. `load_archived_items_returns_entries_from_flat_files` — against the real fixture `docs/spacetop-dev/_archive`. Asserts: count == 3, titles include `Scaffold Rust CLI Project` / `Parse Spacedock Workflow Files` / `Build Initial TUI Overview`, each item has `status == "done"`, each has `Some` `completed` and `Some` `verdict`.
2. `load_archived_items_sorts_by_completed_desc_with_missing_last` — builds a temp `_archive/` directory with three synthesized items having `completed` timestamps `2026-04-24T14:49:53Z`, `2026-04-24T15:00:00Z`, and none. Asserts the ordering is `[T15:00, T14:49, None]`.
3. `load_archived_items_reads_folder_entity_index_md` — temp dir containing `_archive/foo/index.md` plus `_archive/foo/notes.md`. Asserts only the `index.md` is surfaced.
4. `load_archived_items_missing_archive_dir_is_empty_ok` — temp workflow with no `_archive/`. Asserts `Ok(Vec::new())`, no error.
5. `load_archived_items_propagates_parse_error_with_path` — temp `_archive/broken.md` with bad YAML. Asserts the returned `ParseError` includes the file path.
6. Regression guard (already present): `loads_workflow_snapshot_from_directory_ignoring_mods_and_archive` must continue to pass unchanged. Do not edit it.

### App-state tests (`src/app.rs`, `#[cfg(test)] mod tests`)

7. `default_view_scope_is_active_and_visible_items_match_snapshot` — `App::from_snapshot(...)` has `view_scope() == ViewScope::Active` and `visible_items()` points to `snapshot.items`.
8. `toggle_scope_key_a_flips_to_archived_and_loads_lazily` — use `App::load` against the real `docs/spacetop-dev` directory, then send `KeyCode::Char('a')`. Assert `view_scope() == Archived`, `archived_items()` non-empty, `selected_item()` returns an archived entry, and `archived_count() == Some(3)`. A second `a` flips back to `Active` and `visible_items()` again equals `snapshot.items`.
9. `archived_view_selection_is_independent_of_active_selection` — toggle to archived, press `j` twice, toggle back to active. Assert active `selected_index()` still `0`, then toggle to archived again and assert archived selection is still at index 2.
10. `navigation_is_clamped_per_scope` — handcrafted `App` with archived slice of length 1: `End` / `Down` never puts selection out of bounds.
11. `archive_count_hidden_before_first_toggle` — fresh `App`, `archived_count() == None` until the first `a` press.

### UI render tests (`src/ui/mod.rs`, `#[cfg(test)] mod tests`)

12. `active_view_renders_scope_and_archived_placeholder_lines` — build an `App` at default scope. Assert rendered buffer contains `view: active` and `archived: (press a)` (or whichever exact string is chosen in implement — update test to match).
13. `archived_view_preview_renders_verdict_and_completed` — build an `App`, inject a known `archived_items` slice (e.g., copy a fixture's parsed `WorkItem`), set scope to `Archived`. Assert buffer contains `verdict: PASSED`, `completed: 2026-04-24T14:49:53Z`, and the existing `status: done` / `source: ...` / `path: ...` lines.
14. `archived_view_list_appends_verdict_glyph_and_dims_rows` — scope=`Archived`, two items with `verdict = Some("PASSED")` and `verdict = None`. Assert list line 1 ends with `[✓]`, line 2 with `[?]`. (Glyph-presence assertion is enough; ratatui `TestBackend` does not expose style flags ergonomically — if an implement-stage reviewer wants a dim-style assertion, they can probe `buffer.get(x, y).modifier`.)
15. `summary_footer_lists_a_toggle_hint` — assert rendered buffer contains `a toggle archive`.

### Smoke

16. Manual `cargo run -- --workflow-dir docs/spacetop-dev` per Layer 4 above. Record terminal transcript in the implement stage report if the reviewer wants evidence; otherwise `git diff -- docs/spacetop-dev` empty is sufficient evidence of the read-only contract.

## File / module ownership (implement stage)

Worker scope for the implement worktree is confined to the following files. No other source files are expected to change.

| Concern | File(s) | Owner boundary |
|---|---|---|
| Archive loader, sort, folder-entity handling | `src/parser.rs` | Parser layer only; no UI/domain knowledge. |
| Optional `archive_dir(...)` helper | `src/parser.rs` (or `src/domain/mod.rs` if it lives better there) | Prefer co-locating with the loader unless a domain-wide helper already exists. |
| `ViewScope` enum, archive slice, per-scope selection, `a` key handler | `src/app.rs` | App-state layer; imports `load_archived_items` from `parser`. Must not touch ratatui. |
| Summary/list/preview scope branching, verdict glyph, footer hint | `src/ui/mod.rs` | Rendering only; reads state via `App` accessors (`view_scope`, `visible_items`, `archived_count`, `archive_error`). |
| Terminal event loop | `src/lib.rs` | No changes expected. If a change is needed (e.g., to grow summary height), it still lives in `src/ui/mod.rs`, not `lib.rs`. |
| Domain types | `src/domain/mod.rs` | **No changes** — `WorkItem.completed` and `WorkItem.verdict` already exist. |
| CLI surface | `src/cli.rs` | **No changes** — no new flags. |
| Tests | Same-file `#[cfg(test)] mod tests` blocks on the three modules above | Parser tests in `src/parser.rs`; app-state tests in `src/app.rs`; render tests in `src/ui/mod.rs`. |
| Fixtures | `docs/spacetop-dev/_archive/*.md` | **Read-only** — use as-is. Any temp fixtures go under `std::env::temp_dir()`. |

Out of scope for this task (do not edit): `src/main.rs`, `src/cli.rs`, `src/domain/mod.rs`, any file under `agents/`, `skills/`, `references/`, `plugin.json`, or `docs/spacetop-dev/README.md`.

## Verification commands (copy-paste for the implement stage)

```bash
cargo fmt --all
cargo test
cargo run -- --workflow-dir docs/spacetop-dev   # manual smoke: a, j, k, a, q
git diff -- docs/spacetop-dev                    # must be empty
```

## Stage Report: plan

- DONE: Step-by-step implementation plan lists concrete file paths, expected functions/types, and verification commands (cargo fmt, cargo test, cargo run smoke).
  Four-layer plan above names `src/parser.rs` (`load_archived_items`), `src/app.rs` (`ViewScope`, `visible_items`, `ensure_archive_loaded`, `a` handler), `src/ui/mod.rs` (summary/list/preview branching), and `src/lib.rs` (no change); verification block lists `cargo fmt --all`, `cargo test`, `cargo run -- --workflow-dir docs/spacetop-dev`, and the `git diff` read-only check.
- DONE: Focused test strategy names the specific parser, app-state, and render tests to add — separating parser/_archive/ loading, scope-toggle app-state, and preview render assertions.
  Test strategy enumerates 5 parser tests (flat-file load, sort-by-completed, folder-entity index.md, missing-dir OK, malformed propagation), 5 app-state tests (default scope, `a` toggle + lazy load, per-scope selection, clamp, count-hidden-until-toggle), and 4 UI render tests (scope+placeholder lines, verdict/completed preview, verdict glyphs, footer hint).
- DONE: File/module ownership for the implement stage is explicit (parser loader lives under src/parser*/src/domain, App state and ViewScope in src/app.rs, render changes in src/ui/*, terminal wiring in src/lib.rs if needed).
  Ownership table pins parser loader to `src/parser.rs`, `ViewScope`/archive slice/`a` handler to `src/app.rs`, all rendering to `src/ui/mod.rs`, explicitly marks `src/domain/mod.rs` and `src/lib.rs` as "no changes expected", and lists out-of-scope files.

### Summary

Implementation is sliced into four layers the implement stage can land as sequential commits: (1) a read-only `load_archived_items` in `src/parser.rs` that reuses `parse_work_item`, skips missing `_archive/` silently, and sorts newest-first by `completed`; (2) `ViewScope { Active, Archived }` plus a lazy `ensure_archive_loaded` + per-scope selection on `App` in `src/app.rs`; (3) summary/list/preview branching in `src/ui/mod.rs` that adds `view:`/`archived:` lines, verdict glyphs with `Modifier::DIM`, and `verdict:`/`completed:` preview fields; (4) no terminal-loop edits — the existing event loop handles the new `a` key for free. Domain types, CLI, and fixtures are unchanged, preserving the active-snapshot regression test verbatim.

## Stage Report: implement

- DONE: Parser adds `load_archived_items` without regressing `WorkflowSnapshot`'s active-only contract; App adds `ViewScope` + toggle; archived view renders with verdict+completed in preview and proper ordering; graph ribbon header surfaces scope indicator and archived count.
  `src/parser.rs` adds `load_archived_items` + `archive_dir`; `loads_workflow_snapshot_from_directory_ignoring_mods_and_archive` still passes. `src/app.rs` adds `ViewScope { Active, Archived }`, `a` toggle, per-scope selection, lazy `ensure_archive_loaded`. `src/ui/mod.rs` branches preview on scope (adds `verdict:` / `completed:` between `source:` and `path:`) and adds verdict glyphs + DIM on archived rows. `src/ui/graph.rs` now consumes `state.view_scope()` + `state.archived_count()` and renders `[active]`/`[archived]` plus `archived: N` / `archived: (press a)` in the ribbon header title.
- DONE: `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test` clean on the worktree branch (pre-existing errors on main named explicitly if they survive).
  `cargo fmt --check` clean. `cargo test`: 59 passed, 2 pre-existing failures survive identically — `app::tests::loads_real_workflow_state_and_derives_stage_counts` (asserts selected title == "Build Initial TUI Overview" but real workflow now returns task 006 "Show completed (archived) tasks in the TUI") and `ui::tests::renders_real_workflow_summary_task_list_and_preview` (asserts body contains "Build the first read-only"). `cargo clippy --all-targets -- -D warnings` reports only the pre-existing `clippy::unnecessary_lazy_evaluations` at `src/parser.rs` (now line 319 after the new loader; previously 246) — called out in the task spec as out-of-scope.
- DONE: Smoke run `cargo run -- --workflow-dir docs/spacetop-dev` then press `a` to toggle archived view then `q` to quit leaves `git diff -- docs/spacetop-dev` empty (read-only contract preserved).
  Ran via `printf 'aq' | script -q /dev/null cargo run -- --workflow-dir docs/spacetop-dev`. App started, ribbon header rendered, `a` toggled to archived scope, `q` quit. `git diff -- docs/spacetop-dev` returns empty.

### Summary

Layered implementation landed: parser gained `load_archived_items` + `archive_dir` (sorting newest-first by `completed`, folder-entity `index.md` support, missing `_archive/` as empty), `App::OverviewState` gained `ViewScope`, lazy archive load, per-scope selection, and an `a` key toggle, and the UI layer added verdict glyphs / DIM on archived rows, `verdict:` + `completed:` preview fields, and scope+count in the graph ribbon header. `WorkflowSnapshot` stays active-only (regression test unchanged), domain types untouched, read-only contract preserved. Pre-existing main-branch fixture-drift test failures and the pre-existing `unnecessary_lazy_evaluations` clippy lint still surface identically — no incidental fixes, no regressions introduced.
