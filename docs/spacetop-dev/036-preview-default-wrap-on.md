---
id: "036"
title: Default-enable word wrap when opening preview mode
status: review
source: captain
started: 2026-04-27T10:37:06Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-036-preview-default-wrap-on
issue:
pr:
---

When the user opens the entity preview pane, word wrap should be enabled by default. Today `OverviewState` initializes `preview_wrap: false` (`src/app/overview.rs:92`), so long lines extend past the preview pane width and the user has to press `w` every time they open a preview to toggle wrap on.

The expected behavior: opening preview shows wrapped content out of the box. The `w` key still toggles wrap (so power users who prefer horizontal scroll can disable it), and the wrap setting persists for the lifetime of the session — toggling once should not be undone the next time preview is opened.

Reference:

- `src/app/overview.rs:35-92` — `OverviewState` field declaration and constructor; `preview_wrap` default.
- `src/app/keys.rs:75-76` — `w` key calls `state.toggle_preview_wrap()`.
- `src/ui/mod.rs:333,382` — footer/help hints document `w: word wrap`.
- `src/ui/mod.rs:654` — render path branches on `state.preview_wrap()`.

## Acceptance criteria

**AC-1 — A freshly opened preview pane renders with word wrap enabled.**
Verified by: an `app::tests` test that constructs an `OverviewState` (via the same constructor path used by `App::load`), opens the preview, and asserts `state.preview_wrap()` returns `true` without any `toggle_preview_wrap()` call.

**AC-2 — Pressing `w` still toggles wrap and the new value sticks.**
Verified by: an `app::keys::tests` (or equivalent `app::tests`) test that simulates the `w` keypress against an open preview, asserts `preview_wrap()` flips to `false`, simulates `w` again, asserts it flips back to `true`. No regression in existing `preview_open()` / scroll behavior on the same fixture.

**AC-3 — Existing wrap-aware UI assertions and footer hints continue to pass.**
Verified by: `cargo test` is green on the worktree branch (with the new defaults applied) and `make lint` is clean. Any pre-existing failure must be independently reproduced on `main` HEAD before being declared out of scope, per the convention established in 035.

## Implementation plan

### Code changes (in `src/app/overview.rs`)

There are three sites that touch `preview_wrap`. All three must be updated together — flipping only the constructors leaves a latent bug where `reset_preview_scroll` still clobbers a user-toggled value.

1. **`OverviewState::empty` (line 92):** change `preview_wrap: false` to `preview_wrap: true`. This is the empty-workflow constructor used by the picker before a workflow is loaded; keep it consistent with the loaded constructor so behavior is uniform.

2. **`OverviewState::from_snapshot_with_root` (line 132):** change `preview_wrap: false` to `preview_wrap: true`. This is the canonical constructor reached by `OverviewState::load`, `OverviewState::from_snapshot`, and the picker handoff in `App::load`. AC-1 hinges on this site.

3. **`OverviewState::reset_preview_scroll` (line 446):** *remove* the `self.preview_wrap = false;` line entirely. This method is invoked from four callers (line 184 `reload_from_snapshot`, line 287 — likely a similar reload path, line 361 `set_scope_index` on selection change, line 375 `toggle_preview`). Currently every one of these clobbers wrap back to `false`; with the new default that would be wrong (re-opening preview after closing would also clobber a user's "off" choice back to default). The spec explicitly requires the toggled value to persist for the session, so wrap state must be decoupled from scroll resets — the function name only promises a *scroll* reset anyway.

No change to `App::load`, `App::from_snapshot`, the picker handoff, or any reload path is needed: they all funnel through `from_snapshot_with_root` (or carry their own snapshot through `reload_from_snapshot`, which after change 3 no longer touches `preview_wrap`). The `toggle_preview_wrap` accessor and the `w` keybind in `src/app/keys.rs:75-76` are unchanged.

### Test changes (in `src/ui/mod.rs` `tests` module — keeping the existing fixture conventions there)

The current test `word_wrap_resets_when_preview_closed` (`src/ui/mod.rs:1384-1408`) encodes the *old* contract ("wrap resets on pane close" / "wrap stays off on re-open"). It must be rewritten — not deleted — to encode the new contract. Renaming is appropriate.

| AC | Test name | Location | Assertion |
|----|-----------|----------|-----------|
| AC-1 | `preview_wrap_default_on_for_loaded_overview` (new) | `src/app/overview.rs` `#[cfg(test)] mod tests` | Construct `OverviewState::from_snapshot(dir, snapshot)` (the `App::load` path), then assert `state.preview_wrap() == true` *before* any keypress or `toggle_preview()` call. Also assert it after `state.toggle_preview()` (open) — wrap must already be on when preview opens. |
| AC-2 | `word_wrap_toggle_persists_across_preview_open_close` (rename of the existing `word_wrap_resets_when_preview_closed`) | `src/ui/mod.rs` `tests` | Open preview (Enter), assert `preview_wrap() == true` (default-on). Press `w`, assert `false`. Close preview (Enter), assert `false` still — toggle persists across pane close. Re-open (Enter), assert `false` still — toggle persists across re-open. Press `w`, assert `true` — toggle still works in either direction. |
| AC-2 | `word_wrap_persists_across_reload` (new, optional) | `src/app/overview.rs` `tests` | After toggling wrap off, call `reload_from_snapshot` with the same snapshot and assert `preview_wrap() == false`. Guards the `reset_preview_scroll` change against future regressions. |
| AC-3 | (existing tests) | `src/ui/mod.rs` `tests` | Run `cargo test` and confirm `footer_shows_word_wrap_hint_when_preview_open` and the wrap/scroll-clamp tests at line 1370 still pass — they exercise the toggle and the renderer branch at `src/ui/mod.rs:654`, both of which behave the same whether the default is on or off. `make lint` must be clean. |

The existing test at `src/ui/mod.rs:1370` (wrap-mode scroll clamping) starts with no-wrap implicit and toggles wrap on. Under the new default it starts wrap-on; its `toggle wrap off` step still flips correctly. Audit it during implementation — it may need its initial assertion adjusted if it depends on the starting value, but the toggle semantics it exercises are unchanged.

### Worktree ownership notes (for the `implement` stage)

- **In scope to edit:** `src/app/overview.rs` (3 line changes above + new in-module test for AC-1 / optional reload test), `src/ui/mod.rs` (rewrite the `word_wrap_resets_when_preview_closed` test in the existing `tests` module).
- **Read-only / out of scope:** `src/app/keys.rs` (no behavior change — `w` keeps toggling), `src/ui/mod.rs` outside the `tests` module and lines 333/382 (footer/help copy stays "w: word wrap" / "w toggle word wrap" — still accurate when default is on, the toggle continues to exist), the renderer branch at `src/ui/mod.rs:654` (no change — it already reads `state.preview_wrap()`), `src/watcher.rs`, `src/parser.rs`, `src/discovery.rs`, `src/domain/`, `src/ui/graph.rs`, `src/ui/picker.rs`.
- **UI-hint copy decision:** keep existing strings. The dispatch explicitly flagged 333/382 for confirmation; the wording remains accurate because `w` is still a toggle. Changing copy would invalidate the pinned-string convention noted in `CLAUDE.md` for no benefit.
- **Verification commands:** `cargo test` (full suite) and `make lint` (clippy `-D warnings`). Both must be green before the implement stage closes. No release build required.

## Stage Report: plan

- DONE: Implementation plan that names the exact field flip in src/app/overview.rs (preview_wrap: false -> true at line ~92) plus any constructor sites that bypass the default (e.g., from_snapshot_with_root, reload paths) and how each is handled. Three sites identified: `empty` (line 92), `from_snapshot_with_root` (line 132), and `reset_preview_scroll` (line 446 — remove the wrap mutation rather than flip it, so user toggle persists across reloads/closes per spec).
- DONE: Test strategy mapping AC-1 (default-on at construction), AC-2 (`w` key toggle still flips and persists), AC-3 (cargo test green, make lint clean), with target test file locations and the assertions each test will make. Table above maps each AC to a named test, file location, and the exact assertions; flagged the existing `word_wrap_resets_when_preview_closed` test at `src/ui/mod.rs:1384` as a rename-and-rewrite (it encodes the old "wrap resets on close" contract) and added an optional reload-persistence test guarding the `reset_preview_scroll` change.
- DONE: Module/file ownership notes for the implement worktree: which files implement vs test code may touch (overview.rs, keys.rs, ui hints, in-module tests), what stays out of scope (rendering layout, watcher, parser), and any UI-hint copy that must stay accurate when wrap defaults to on. Explicit in-scope/out-of-scope split listed; confirmed footer/help copy at `src/ui/mod.rs:333,382` stays as-is because `w` is still a toggle and the strings remain accurate.

### Summary

Planned a three-site change in `src/app/overview.rs` (two constructor defaults plus removal of the wrap-clobber inside `reset_preview_scroll`) so preview opens wrapped by default while a user `w` toggle persists across pane close, scope change, and watcher reload. Test plan covers AC-1 via a new `app::overview::tests` constructor assertion, AC-2 by rewriting the existing `word_wrap_resets_when_preview_closed` test to assert toggle-persistence in both directions, and AC-3 via the standard `cargo test` + `make lint` gate.
