---
id: "036"
title: Default-enable word wrap when opening preview mode
status: design
source: captain
started:
completed:
verdict:
score:
worktree:
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
