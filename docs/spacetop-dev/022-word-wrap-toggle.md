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
