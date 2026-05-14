---
id: 039
title: Open entity file from preview (o keybind + OSC 7)
status: design
source: captain conversation — preview pane shows long markdown content that's hard to read in the TUI; absolute paths don't fit the header, so users can't Cmd+click them either
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

When the preview pane is open on a work item, the user has no quick way to open that item's markdown file in a real editor. The full absolute path in the header (`src/ui/mod.rs:858`) gets truncated, which also defeats iTerm2/Ghostty Smart Selection on the rendered text. ratatui doesn't support inline OSC 8 hyperlinks reliably, so embedding clickable links inside the buffer isn't viable.

Two complementary affordances are proposed:

1. **`o` keybind** — when the preview is open, pressing `o` suspends the TUI, opens the entity's markdown file via `$EDITOR` (falling back to `open` on macOS / `xdg-open` on Linux), and resumes the TUI on exit. This is the primary mechanism — it works in every terminal, including over SSH, and doesn't depend on the path being visible.

2. **OSC 7 at startup** — emit `\e]7;file://{host}{cwd}\e\\` once during terminal setup so terminals that support Smart Selection on relative paths can resolve them against the right working directory. Pair with rendering a *relative* path (relative to the workflow root) in the preview header so it fits and stays clickable for mouse users.

Design should resolve:
- which key (`o` confirmed in conversation, but check for conflicts with existing bindings)
- editor selection precedence: `$VISUAL` → `$EDITOR` → platform default opener
- whether to block on the editor or background it (block is simpler and matches lazygit/gitui conventions)
- where to place the OSC 7 emit (before `EnterAlternateScreen`? after?) and whether to guard with `TERM`/`TERM_PROGRAM` checks
- header path rendering: relative-to-workflow-root vs current absolute, and how to handle worktree-resident entities whose paths sit outside the workflow root

## Acceptance criteria

**AC-1 — Pressing `o` in the preview opens the entity file in an external editor.**
Verified by: integration test that drives `App` through `AppMode::Overview` with a preview-open state, simulates `KeyCode::Char('o')`, and asserts the app records an "open file" intent against the selected entity's `path`. (The actual `Command::spawn` may be stubbed behind a trait so the test runs without an editor installed.)

**AC-2 — TUI suspends cleanly around the editor invocation and resumes without artifact.**
Verified by: manual confirmation that `disable_raw_mode` + `LeaveAlternateScreen` happen before the editor spawns and the inverse runs on return, plus a unit test on the terminal-suspend/resume helper that asserts the call sequence.

**AC-3 — `o` is a no-op (or visibly ignored) when the preview is closed.**
Verified by: unit test on the keybind dispatcher that asserts no open-file intent is recorded when `preview_open()` is false.

**AC-4 — Editor resolution falls back deterministically.**
Verified by: unit test on the resolver that exercises `$VISUAL` set, `$EDITOR` set, both unset (platform default), and returns the expected command in each case.

**AC-5 — OSC 7 is emitted exactly once during startup, before the alt screen is entered, and only when stdout is a TTY.**
Verified by: unit test on the startup helper using a captured writer; asserts the bytes match `\e]7;file://<host><urlencoded-cwd>\e\\` and that nothing is emitted when the writer is flagged as non-TTY.

**AC-6 — Help screen documents the new keybinding.**
Verified by: grep for `o` in the help/keybinding list rendered by `render_help_lines` (or equivalent in `src/ui/mod.rs`).

**AC-7 — `make lint` passes.**
Verified by: `make lint` exits zero with no clippy warnings.
