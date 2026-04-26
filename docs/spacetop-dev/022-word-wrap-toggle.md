---
id: "022"
title: "Add word-wrap toggle key in the preview pane"
status: plan
source: feature request
started: 2026-04-26T04:32:47Z
completed:
verdict:
score: 0.6
worktree:
issue:
pr:
---

The preview pane currently renders content with a fixed line-width behavior. Users working with long entity body text or stage reports want to toggle word-wrap on and off without leaving the TUI. A dedicated keybinding should switch between wrapped and unwrapped (horizontal-scroll) mode in the preview pane.

## Acceptance criteria

**AC-1 -- Toggle key switches wrap mode.**
Pressing the designated key (e.g. `w`) while the preview pane is focused toggles between word-wrap and no-wrap (horizontal scroll) modes. The pane re-renders immediately.
Verified by: integration/snapshot test or manual test recording showing both states.

**AC-2 -- Wrap state persists while the pane is focused but resets to default on pane switch.**
Switching away from the preview pane and back resets wrap to the default state (no-wrap), so behavior is predictable.
Verified by: unit test or manual test of pane-switch sequence.

**AC-3 -- Key is discoverable in the help bar or key legend.**
The toggle key appears in the existing help/legend UI element (footer or overlay) when the preview pane is active.
Verified by: screenshot or snapshot test showing the key hint.

## Stage Report: design

- DONE: Problem statement names the exact app state field and TUI render path that must change to support wrap/no-wrap mode.
  `OverviewState.preview_wrap: bool` (src/app.rs) is the new field; `render_preview()` in src/ui/mod.rs (line 481) is the render path — the body `Paragraph` gains `.wrap(Wrap { trim: false })` when the flag is true, and `reset_preview_scroll()` resets it to `false` on item or pane switch.
- DONE: Acceptance criteria confirm the key choice (e.g. w) does not conflict with existing keybindings.
  Existing char bindings are `?`, `q`, `j`, `k`, `a`, `P`; `w` is unbound in all three `AppMode` arms (Overview, Picker, PickerOverlay) — confirmed by grep of `KeyCode::Char` in src/app.rs.

### Summary

The feature requires one new bool field (`preview_wrap`) on `OverviewState`, toggled by `w` only while `preview_open` is true, reset to `false` inside `reset_preview_scroll()`. The render change is a conditional `.wrap(Wrap { trim: false })` on the body `Paragraph` in `render_preview()`; when wrap is active, `max_preview_scroll_x` must be set to 0 so horizontal scroll keys become inert. The key `w` is confirmed free of conflicts. The help footer and help popup in `render_status_footer` and `render_help_popup` (src/ui/mod.rs) must add the `w: word wrap` hint in the `preview_open` conditional branches.
