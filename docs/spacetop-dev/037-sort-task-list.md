---
id: 037
title: Sort the task list by ID or by status
status: review
source: captain request 2026-05-12
score:
worktree: .worktrees/spacedock-ensign-037-sort-task-list
issue:
pr: #30
started: 2026-05-12T07:16:26Z
mod-block: merge:pr-merge
---

The SpaceTop overview lists tasks in whatever order discovery happens to return. Users should be able to choose the sort order so they can scan the list either by stable identifier or by where each task currently sits in the workflow. Provide at minimum two sort modes — sort by `id` and sort by `status` (stage order as declared in the workflow README) — with a clear way to toggle between them from the TUI and a visible indicator of the active sort.

## Acceptance criteria

**AC-1 -- Task list can be sorted by ID.**
Verified by: a unit/integration test in `src/app.rs` (or wherever overview state lives) that asserts the rendered task order matches ascending `id` after selecting the ID sort mode, across a fixture with mixed-status tasks.

**AC-2 -- Task list can be sorted by status (workflow stage order).**
Verified by: a test asserting the rendered task order groups tasks by status using the stage ordering declared in the workflow README (not alphabetical on the status string), with a documented tiebreaker (e.g., ID ascending) for tasks sharing a status.

**AC-3 -- The active sort mode is toggleable from the TUI and visible to the user.**
Verified by: a keybinding documented in the overview UI (and reflected in `src/ui/`) cycles between sort modes, and the header or status line shows which mode is active. Snapshot-style assertion or rendered-string test confirms the indicator changes when the mode changes.

**AC-4 -- Sort behavior is read-only and does not mutate workflow files.**
Verified by: the sort is implemented purely in app state / rendering with no writes to `docs/spacetop-dev/`; covered by the existing read-only invariant tests or a new assertion that no filesystem writes occur on sort toggle.

## Implementation plan

### Design boundary

Sort is a presentation concern over `OverviewState`. The on-disk `WorkflowSnapshot.items` order stays untouched (read-only invariant preserved); sort produces an ordered view computed at read time. State logic lives in `src/app/overview.rs` so it is fully testable without a terminal backend; only the keybinding wiring and the visible indicator live in `src/app/keys.rs` and `src/ui/mod.rs`.

### Step 1 — Add `SortMode` to state (`src/app/overview.rs`)

- Introduce `pub enum SortMode { Id, Status }` with `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]`. Default is `Id` (matches AC-1's "ascending id" semantic and gives the new feature a stable initial mode that does not depend on workflow definition).
- Add `sort_mode: SortMode` field to `OverviewState`. Initialize to `SortMode::default()` in every constructor (`empty`, `from_snapshot_with_root`). Preserve `sort_mode` across `reload_from_snapshot` (do not reset).
- Add accessors `pub fn sort_mode(&self) -> SortMode` and `pub fn cycle_sort_mode(&mut self)` (Id → Status → Id). Cycling resets `preview_scroll` only if selection changes (reuse `reset_preview_scroll` via `set_scope_index` path); selection is preserved by slug across the re-sort, mirroring the `reload_from_snapshot` behavior.
- Replace direct access to `snapshot.items` for the active scope with a new `visible_items()` that returns `&[WorkItem]` from an internally cached, re-sorted `Vec<WorkItem>`. Two options; choose **Option B** for simpler ownership:

  - **Option A (refs):** return `Vec<&WorkItem>`. Forces signature changes throughout `src/ui/mod.rs` (`build_task_list_items`, `selected_item`, etc.).
  - **Option B (cached vec, chosen):** maintain `sorted_active: Vec<WorkItem>` populated whenever `snapshot` or `sort_mode` changes. `visible_items()` returns `&sorted_active` for `ViewScope::Active` and `&archived_items` for `ViewScope::Archived` (archived stays unsorted for this phase; AC-1/AC-2 are scoped to the active task list per the entity body).

  Add a private `fn rebuild_sorted_active(&mut self)` invoked from: every constructor after assigning `snapshot`, `reload_from_snapshot` (after `self.snapshot = snapshot`), and `cycle_sort_mode`.
- Sort logic (all inside `rebuild_sorted_active`):
  - `SortMode::Id`: clone items, then `sort_by(|a, b| compare_ids(&a.id, &b.id))`. `compare_ids` parses leading digits with `str::parse::<u64>` and orders numerically; on parse failure, fall back to lexical `Ord`. Same numeric ID is a tie broken lexically (stable).
  - `SortMode::Status`: clone items, then `sort_by_key` with a tuple `(stage_index, id_key)` where `stage_index` comes from `snapshot.definition.stages.iter().position(|s| s.name == item.status)`, and unknown statuses sort to the end (`stages.len()`). `id_key` reuses the same numeric/lexical comparator (documented tiebreaker = ID ascending, per AC-2).
- Selection-preservation during `cycle_sort_mode`: record `slug_of(selected_item.path)` before rebuild, then re-find the same slug after rebuild and update `selected_index` (mirror the `reload_from_snapshot` pattern). On miss, clamp to last.

### Step 2 — Wire the keybinding (`src/app/keys.rs`)

- Add a match arm for `KeyCode::Char('s')` (and only `'s'`, no Shift modifier) that calls `state.cycle_sort_mode()` and returns `OverviewKeyAction::None`. Place it before the multi-session arms so it works in both single and multi sessions. Skipped while `state.preview_open()` is true (consistent with how `'w'` is gated).
- No new `OverviewKeyAction` variant needed — the cycle is pure state mutation.

### Step 3 — Render the active-mode indicator (`src/ui/mod.rs`)

- In `render_header_bar`, after the `[active]/[archived]` scope badge, append a second badge `[sort: id]` / `[sort: status]` (dim styling, no background fill — matches archived-mode badge style) with the standard "(press s)" key hint. Recompute `prefix_len`/`used` to include the new badge and hint widths so path truncation stays correct.
- Add `"s: sort"` pill to `status_footer_hints` (active scope only — keep it out of the archived-only paths by leaving the hint unconditional; the keybinding handler simply does nothing when there are zero items).
- Add `"  s              cycle sort mode (id / status)"` to the help popup body in `render_help_popup`. Bump the `popup_h` clamp by 1 line in each branch.

### Step 4 — Tests

Tests live next to the code they exercise so the parser/state/UI separation is preserved.

`src/app/overview.rs` `#[cfg(test)] mod tests`:

1. `sort_by_id_orders_ascending_across_mixed_status` — build a snapshot with items ids `["010", "002", "037"]` and varied statuses; assert `state.visible_items()` returns ids `["002", "010", "037"]` under `SortMode::Id`. (AC-1)
2. `sort_by_status_uses_workflow_stage_order` — build a definition with stages `[design, plan, implement, done]` (intentionally non-alphabetical) and items spread across them; cycle to `SortMode::Status`; assert the rendered order groups items by stage in the declared sequence, with ID-ascending tiebreak inside a stage. (AC-2)
3. `sort_by_status_pushes_unknown_status_to_end` — include one item with a status not in `definition.stages`; assert it lands after all known-status items.
4. `cycle_sort_mode_preserves_selection_by_slug` — select item id `010`, cycle from Id→Status; assert `selected_item().id == "010"`.
5. `cycle_sort_mode_default_and_cycles_back` — assert `sort_mode()` starts at `Id`, cycles `Id → Status → Id`.
6. `reload_from_snapshot_preserves_sort_mode` — set mode to `Status`, reload, assert mode is still `Status` and `visible_items()` is re-sorted by status.

`src/app/tests.rs` (or a new module if needed) — keybinding integration:

7. `pressing_s_cycles_sort_mode` — drive `App::handle_key(KeyCode::Char('s'))` against an `App::from_snapshot` and assert `as_overview().sort_mode()` changes accordingly. Verifies the wire-up in `keys.rs`.

`src/ui/mod.rs` `#[cfg(test)] mod tests` (or add `tests/sort_indicator.rs`):

8. `header_bar_shows_sort_badge` — render `render_header_bar` into a `TestBackend` (already used by other UI tests; if none, add a minimal one), assert the rendered buffer contains `[sort: id]` initially and `[sort: status]` after cycling. Covers AC-3's "header shows which mode is active".

Read-only invariant for AC-4:

9. In `src/app/overview.rs` tests, after cycling sort modes N times, assert `state.snapshot().items` (the on-disk-derived ordering) is bitwise-unchanged. No filesystem mock is needed because sort never touches the FS by construction; this assertion is the codified invariant.

### Step 5 — Verification commands

Run, in order, from the worktree (implement stage will own a worktree per workflow norms; the plan stage just documents the verify recipe):

```
cargo test sort_by_id_orders_ascending_across_mixed_status
cargo test sort_by_status_uses_workflow_stage_order
cargo test sort_by_status_pushes_unknown_status_to_end
cargo test cycle_sort_mode_preserves_selection_by_slug
cargo test cycle_sort_mode_default_and_cycles_back
cargo test reload_from_snapshot_preserves_sort_mode
cargo test pressing_s_cycles_sort_mode
cargo test header_bar_shows_sort_badge
cargo test                       # full suite — confirms no regressions in existing overview/ui tests
make lint                        # clippy with -D warnings gate (CLAUDE.md requirement)
```

Each AC maps to test names:

- AC-1: test 1.
- AC-2: tests 2, 3.
- AC-3: tests 5, 7, 8.
- AC-4: test 9 (and the structural fact that sort lives in `src/app/overview.rs` with no FS calls).

### Module / file ownership for the implementer worktree

- Edit: `src/app/overview.rs` (state, sort logic, tests 1-6, 9), `src/app/keys.rs` (keybinding, no new tests needed beyond test 7), `src/app/tests.rs` (test 7), `src/ui/mod.rs` (header badge, footer hint, help popup, test 8).
- Do not touch: `src/parser/`, `src/discovery.rs`, `src/watcher.rs`, `src/domain/` (sort is a view concern, not a parsing or domain concern).
- Do not modify `docs/spacetop-dev/` outside of the entity's stage report — preserves AC-4 / CLAUDE.md read-only invariant.

## Stage Report: plan

- DONE: Plan separates sort logic in app state from TUI rendering so it can be tested without a terminal backend.
  Step 1 places `SortMode`, the cached `sorted_active` view, and `cycle_sort_mode` in `src/app/overview.rs`; UI changes (Step 3) only read `sort_mode()` and consume `visible_items()`. Tests 1-6, 9 run without a terminal backend.
- DONE: Plan names the concrete keybinding to cycle sort modes and where the active-mode indicator renders in the overview UI.
  Keybinding: `s` (Step 2, `src/app/keys.rs`, gated off when preview is open like `w`). Indicator: a second header badge `[sort: id]` / `[sort: status]` in `render_header_bar` plus an `s: sort` pill in the footer hints and a help-popup entry (Step 3, `src/ui/mod.rs`).
- DONE: Plan lists the test commands (cargo test target plus `make lint`) that will prove each AC in the entity body.
  Step 5 lists nine targeted `cargo test` invocations plus `cargo test` (full suite) and `make lint`, with each AC mapped to the test(s) that prove it.

### Summary

Sort is implemented as a presentation layer over `OverviewState` via a `SortMode` enum and a cached `sorted_active` vec rebuilt on snapshot/mode change; numeric ID comparator with lexical fallback is reused as the status-tier tiebreaker. Keybinding `s` cycles modes (default `Id`); a header badge and footer pill expose the active mode. Read-only invariant is preserved by construction — `snapshot.items` is never mutated — and verified by test 9 in addition to the structural choice of locating all sort logic in `src/app/overview.rs`.

