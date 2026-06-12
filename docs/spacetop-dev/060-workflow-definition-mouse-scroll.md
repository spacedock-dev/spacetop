---
id: "060"
title: Workflow Definition page supports mouse scroll wheel
status: implement
source: captain request 2026-06-12
kind: feature
risk: low
milestone: v1-maintenance
proof: app/input and UI regression plus make lint
started: 2026-06-12T12:55:32Z
completed:
verdict:
score: 0.62
worktree: .worktrees/spacedock-ensign-060-workflow-definition-mouse-scroll
issue:
pr:
---

The Workflow Definition page should support mouse scroll wheel input, matching
the scroll behavior users expect from the task preview and other scrollable
panels. A user inspecting the workflow README/definition should be able to scroll
through longer definitions with the mouse wheel without switching back to
keyboard-only navigation.

## Scope

- Kind: feature
- Risk: low
- Milestone: v1-maintenance
- Touches: app-state / UI
- Non-goals: changing workflow definition parsing, changing task-list mouse
  behavior, adding workflow markdown write support

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 -- Workflow Definition view handles mouse wheel scrolling.**
When the Workflow Definition page is open and its content exceeds the visible
height, mouse wheel up/down adjusts the definition scroll offset.
Verified by:

**AC-2 -- Mouse wheel behavior stays scoped to the active page.**
Mouse wheel input in the Workflow Definition page does not accidentally scroll
the task list, preview pane, picker, or help overlay state.
Verified by:

**AC-3 -- Keyboard behavior remains unchanged.**
Existing keyboard controls for opening, closing, and scrolling the Workflow
Definition page continue to work as documented.
Verified by:

## Proof plan

- Lowest test layer: app/input tests for mouse wheel events on the Workflow
  Definition page, plus Ratatui rendering assertions if visible scroll state is
  exposed in the rendered page.
- Required command: `make lint`
- Manual check, if any: run `cargo run -p spacetop -- --workflow-dir docs/spacetop-dev`,
  open the Workflow Definition page, and scroll it with the mouse wheel.
- Docs/policy update needed: update help/footer text only if the user-facing
  mouse behavior is documented there.

## Implementation plan

### Ownership and files

App/input state owns the mouse-wheel event semantics. Rendering owns only the
visible definition body offset. Do not move hit testing, parsing, or workflow
schema logic into the UI renderer.

Planned edits:

- Modify `crates/spacetop/src/app.rs`
  - Update `App::handle_mouse` so the existing `help_open` guard still returns
    first.
  - Add a dedicated `AppMode::Definition { scroll, .. }` branch that consumes
    only `MouseEventKind::ScrollDown` and `MouseEventKind::ScrollUp`.
  - Use the same rows-per-wheel-notch value as overview preview wheel scrolling.
  - Leave `Search`, `Timeline`, `Metrics`, `Activity`, and `Relations` mouse
    behavior inert.
  - Leave `Picker` and `PickerOverlay` click handling exactly on the existing
    `handle_picker_mouse` path.
- Modify `crates/spacetop/src/app/mouse.rs`
  - Make the current wheel row constant reusable inside the app module, for
    example `pub(crate) const WHEEL_SCROLL_ROWS: isize = 3;`.
  - Keep `handle_overview_mouse`, `overview_hit`, and task-list wheel behavior
    unchanged.
- Modify `crates/spacetop/src/app/tests.rs`
  - Add app/input tests beside the existing "Definition view tests (task 041)"
    block.
  - Proposed test names:
    - `definition_mouse_wheel_scrolls_definition_view`
    - `definition_mouse_wheel_is_ignored_while_help_open`
    - `definition_mouse_wheel_does_not_touch_underlying_overview_state`
- Modify `crates/spacetop/src/ui/definition.rs`
  - Add one Ratatui `TestBackend` regression in the module-local tests:
    `definition_scroll_offset_moves_prose_body`.
  - Keep `render_in`'s contract as `(&WorkflowDefinition, scroll)`; do not make
    rendering own input events.
- No change expected in `README.md`, footer, or help text. The existing help
  string "Wheel          scroll panel under cursor" remains accurate; update its
  pinned test only if the implementation changes that user-facing copy.

### Implementation steps

1. In `crates/spacetop/src/app/mouse.rs`, expose the existing wheel step:

   ```rust
   /// Rows moved per wheel notch over scrollable body panels.
   pub(crate) const WHEEL_SCROLL_ROWS: isize = 3;
   ```

2. In `crates/spacetop/src/app.rs`, update `App::handle_mouse` with a Definition
   branch before the catch-all inert full-pane branch:

   ```rust
   AppMode::Definition { scroll, .. } => {
       match mouse.kind {
           crossterm::event::MouseEventKind::ScrollDown => {
               *scroll = scroll.saturating_add(mouse::WHEEL_SCROLL_ROWS as usize);
           }
           crossterm::event::MouseEventKind::ScrollUp => {
               *scroll = scroll.saturating_sub(mouse::WHEEL_SCROLL_ROWS as usize);
           }
           _ => {}
       }
       return;
   }
   ```

   Keep clamping at render time, matching the current keyboard Definition
   scroll model. Do not add workflow-content reads, writes, or parser calls.

3. In `crates/spacetop/src/app/tests.rs`, add a small mouse helper near the
   Definition tests:

   ```rust
   fn mouse(kind: crossterm::event::MouseEventKind) -> crossterm::event::MouseEvent {
       crossterm::event::MouseEvent {
           kind,
           column: 0,
           row: 0,
           modifiers: KeyModifiers::NONE,
       }
   }
   ```

   Test `definition_mouse_wheel_scrolls_definition_view`:
   - Build `App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2))`.
   - Press `D`.
   - Send `ScrollDown` and assert `definition_scroll() == Some(3)`.
   - Send `ScrollUp` and assert `definition_scroll() == Some(0)`.

   Test `definition_mouse_wheel_is_ignored_while_help_open`:
   - Press `D`, then `?`.
   - Send `ScrollDown`.
   - Assert `help_open()` is still true and `definition_scroll() == Some(0)`.

   Test `definition_mouse_wheel_does_not_touch_underlying_overview_state`:
   - Start from overview, move selection down once, capture `selected_index`,
     `view_scope`, `sort_mode`, and `preview_scroll`.
   - Press `D`, send two `ScrollDown` events, then `Esc`.
   - Assert the captured overview state is restored unchanged.

4. In `crates/spacetop/src/ui/definition.rs`, add
   `definition_scroll_offset_moves_prose_body`:
   - Use `five_stage_fixture` with `plan` prose containing lines
     `line-00` through `line-30`.
   - Render with `render_in(frame, frame.area(), &definition, 0)` in a small
     `TestBackend`, assert an early line is visible.
   - Render again with `scroll = 8`, assert the early line is no longer visible
     and a later line is visible.
   - This proves the app-state offset is reflected by the full-pane Definition
     renderer without moving input logic into rendering.

### AC coverage and verification

| Acceptance criterion | Regression coverage | Commands |
| --- | --- | --- |
| AC-1 -- Workflow Definition view handles mouse wheel scrolling | New `definition_mouse_wheel_scrolls_definition_view` app/input test plus `definition_scroll_offset_moves_prose_body` UI render test | `cargo test -p spacetop definition_mouse_wheel`; `cargo test -p spacetop definition_scroll_offset_moves_prose_body` |
| AC-2 -- Mouse wheel behavior stays scoped to the active page | New `definition_mouse_wheel_does_not_touch_underlying_overview_state`; existing `app::mouse::tests::wheel_targets_panel_under_cursor` protects overview list/preview wheel behavior; new help-open test protects overlay state | `cargo test -p spacetop definition_mouse_wheel`; `cargo test -p spacetop app::mouse::tests::wheel_targets_panel_under_cursor` |
| AC-3 -- Keyboard behavior remains unchanged | Existing Definition keyboard tests remain the guard: `d_from_overview_enters_definition_mode`, `esc_from_definition_restores_overview_state`, `d_inside_definition_closes_the_view`, `d_ignored_when_preview_open`, `scroll_keys_advance_definition_scroll`, and multi-workflow Definition tests | `cargo test -p spacetop definition` |

Completion commands for the implement stage:

```bash
cargo fmt --check
cargo test -p spacetop definition
cargo test -p spacetop app::mouse::tests::wheel_targets_panel_under_cursor
make lint
```

Manual TUI check remains a recommended final smoke, not a substitute for the
regression suite:

```bash
cargo run -p spacetop -- --workflow-dir docs/spacetop-dev
```

Open the Workflow Definition page with `D`, scroll with the mouse wheel, then
confirm `Esc` returns to the unchanged overview. This manual check is useful
because Ratatui `TestBackend` cannot prove a local terminal emulator delivers
wheel events, but the automated tests are the CI-quality proof.

### Non-goals preserved

- No workflow definition parsing change: do not edit
  `crates/spacetop-core/src/parser.rs`,
  `crates/spacetop-core/src/parser/readme.rs`, or
  `crates/spacetop-core/src/domain/mod.rs`.
- No task-list mouse behavior change: keep `overview_hit`,
  `handle_overview_mouse`, and the existing list/preview wheel tests
  behaviorally unchanged except for sharing the wheel-step constant.
- No workflow markdown write support: do not add `fs::write`, markdown mutation,
  or git write paths. Spacetop remains read-first; this plan-stage entity update
  is workflow-process state, not product write support.

## Stage Report: plan

- DONE: Plan names exact modules/files/tests for Workflow Definition mouse-wheel handling and distinguishes app-state versus UI rendering ownership.
  See `## Implementation plan` -> `Ownership and files`, which assigns input to `app.rs`/`app/mouse.rs` and rendering proof to `ui/definition.rs`.
- DONE: Plan maps AC-1 through AC-3 to regression tests/commands, including `make lint` and whether a manual TUI check remains necessary.
  See `## AC coverage and verification`; it names per-AC tests, targeted `cargo test` commands, `make lint`, and the recommended manual TUI smoke.
- DONE: Plan preserves non-goals: no workflow definition parsing change, no task-list mouse behavior change, and no workflow markdown write support.
  See `## Non-goals preserved`, which explicitly excludes parser/domain changes, task-list mouse changes, and workflow markdown writes.

### Summary

Planned a scoped app-state mouse-wheel addition for `AppMode::Definition` with a
UI regression proving the existing Definition renderer responds to scroll
offsets. The plan keeps parsing and workflow writes out of scope, leaves overview
task-list mouse behavior intact, and makes `make lint` the required completion
gate for the implement stage.
