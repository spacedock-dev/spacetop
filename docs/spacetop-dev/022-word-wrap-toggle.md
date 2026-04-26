---
id: "022"
title: "Add word-wrap toggle key in the preview pane"
status: review
source: feature request
started: 2026-04-26T04:32:47Z
completed:
verdict:
score: 0.6
worktree: .worktrees/spacedock-ensign-022-word-wrap-toggle
issue:
pr:
mod-block: merge:pr-merge
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

## Implementation Plan

### Overview

Five touch points, three files (`src/app.rs`, `src/ui/mod.rs`, and existing test module inside `src/ui/mod.rs`). No new crate dependencies; `ratatui::widgets::Wrap` is already imported at line 12.

---

### Step 1 — Add `preview_wrap: bool` field to `OverviewState` (src/app.rs)

**File:** `src/app.rs`

1. In the `OverviewState` struct (line 25), add the field after `max_preview_scroll_x`:
   ```rust
   pub preview_wrap: bool,
   ```
2. In `OverviewState::empty()` (line 61), initialize `preview_wrap: false`.
3. In `OverviewState::from_snapshot_with_root()` (line 106), initialize `preview_wrap: false`.
4. Add accessor and toggle methods to the `impl OverviewState` block (after `preview_scroll_x()` at ~line 372):
   ```rust
   pub fn preview_wrap(&self) -> bool {
       self.preview_wrap
   }

   pub fn toggle_preview_wrap(&mut self) {
       self.preview_wrap = !self.preview_wrap;
   }
   ```
5. In `reset_preview_scroll()` (line 422), add `self.preview_wrap = false;` as the last line.

**Evidence command:** `cargo test -q 2>&1 | tail -5` — all existing tests must still pass after this step.

---

### Step 2 — Wire key handler for `w` while preview is open (src/app.rs)

**File:** `src/app.rs`, `AppMode::Overview` match arm (line 993–1021).

Insert before the final `_ => {}` arm:
```rust
KeyCode::Char('w') if state.preview_open() => state.toggle_preview_wrap(),
```

Place it after the existing `KeyCode::Right if state.preview_open()` arm (line 1007) for grouping consistency.

**Evidence command:** `cargo test -q 2>&1 | tail -5`

---

### Step 3 — Conditional wrap in `render_preview()` (src/ui/mod.rs)

**File:** `src/ui/mod.rs`, function `render_preview()` starting at line 481.

Current body render (line 564–567):
```rust
frame.render_widget(
    Paragraph::new(body_lines).scroll((scroll_position as u16, scroll_x)),
    body_area,
);
```

Replace with:
```rust
let body_para = if state.preview_wrap() {
    state.max_preview_scroll_x.set(0);
    Paragraph::new(body_lines)
        .scroll((scroll_position as u16, 0))
        .wrap(Wrap { trim: false })
} else {
    Paragraph::new(body_lines).scroll((scroll_position as u16, scroll_x))
};
frame.render_widget(body_para, body_area);
```

Note: `state.max_preview_scroll_x.set(0)` is set inside the wrap branch so horizontal scroll keys (right/left) become inert while wrap is active. The `max_scroll_x` calculation at line 560 still runs first; the override here shadows it only in the wrapped render path.

**Evidence command:** `cargo test -q 2>&1 | tail -5`

---

### Step 4 — Footer legend update in `render_status_footer()` (src/ui/mod.rs)

**File:** `src/ui/mod.rs`, function `render_status_footer()` at line 275.

Inside the `if preview_open {` block (line 278), after the existing `PgUp/PgDn: preview scroll` hint (line 289), add the wrap hint. The updated block:

```rust
if preview_open {
    hints.push("\u{2190}/\u{2192}: preview scroll");
    hints.push("w: word wrap");   // ADD THIS LINE
} else if session.is_multi() {
    hints.push("\u{2190}/\u{2192}: switch workflow");
}
```

And in the second `if preview_open` block (line 288):
```rust
if preview_open {
    hints.push("PgUp/PgDn: preview scroll");
    // keep as is — the "w: word wrap" entry was already added above
}
```

Actually only one insertion is needed: add `hints.push("w: word wrap");` inside either the first or second `if preview_open` block. The footer is a single joined string so placement in either block works; add it in the second block after `PgUp/PgDn: preview scroll` for visual grouping with other preview hints.

**Evidence command:** `cargo test -q 2>&1 | tail -5`

---

### Step 5 — Help popup update in `render_help_popup()` (src/ui/mod.rs)

**File:** `src/ui/mod.rs`, function `render_help_popup()` at line 303.

Inside the first `if preview_open {` block (line 335–341) that adds PageUp/PageDown lines, add a wrap hint line. After the existing lines:
```rust
if preview_open {
    lines.push(Line::from("  PageUp         scroll preview up"));
    lines.push(Line::from("  PageDown       scroll preview down"));
    lines.push(Line::from("  w              toggle word wrap"));  // ADD THIS LINE
}
```

Also update `popup_h` sizing at line 310: the current `if is_multi { 22 } else { 18 }` may need one extra row when `preview_open` adds lines. Check if `18` remains sufficient; if the popup clips, raise the non-multi cap from `18` to `20`. This needs verification after rendering.

**Evidence command:** `cargo test -q 2>&1 | tail -5`

---

### Step 6 — Tests (src/ui/mod.rs, `#[cfg(test)]` module)

Add three unit/integration tests inside the existing `mod tests` block at the end of `src/ui/mod.rs`.

#### Test AC-1: toggle re-renders body in wrap mode

```rust
#[test]
fn word_wrap_toggle_changes_body_render() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    // Build an app with a long body line that would overflow in no-wrap mode.
    let mut app = App::from_snapshot(
        PathBuf::from("/tmp/ww-ac1"),
        snapshot_with_body("001", "Wrap test", &"a".repeat(200)),
    );
    // Open preview.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
    // No-wrap state: horizontal scroll is live (max_scroll_x > 0 after render).
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let state = app.as_overview().unwrap();
    let max_x_before = state.max_preview_scroll_x.get();
    // Toggle wrap on.
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let state = app.as_overview().unwrap();
    assert_eq!(state.max_preview_scroll_x.get(), 0, "wrap mode clamps scroll_x to 0");
    // Toggle wrap off.
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let state = app.as_overview().unwrap();
    assert_eq!(state.max_preview_scroll_x.get(), max_x_before, "no-wrap restores scroll_x");
}
```

Helper `snapshot_with_body` produces a `WorkflowSnapshot` containing one item with a given body; add it alongside the existing `item()` helper.

#### Test AC-2: wrap resets on pane switch (toggle_preview or select_next)

```rust
#[test]
fn word_wrap_resets_when_preview_closed() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = App::from_snapshot(
        PathBuf::from("/tmp/ww-ac2"),
        snapshot_with_body("001", "Reset test", "some body"),
    );
    // Open preview, enable wrap.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    assert!(app.as_overview().unwrap().preview_wrap());
    // Close preview — wraps reset via toggle_preview → reset_preview_scroll.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.as_overview().unwrap().preview_wrap(), "wrap resets on pane close");
    // Re-open: default should be no-wrap.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.as_overview().unwrap().preview_wrap(), "wrap stays off on re-open");
}
```

#### Test AC-3: legend visible in footer when preview open

```rust
#[test]
fn footer_shows_word_wrap_hint_when_preview_open() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = App::from_snapshot(
        PathBuf::from("/tmp/ww-ac3"),
        snapshot_with_body("001", "Legend test", "body"),
    );
    let mut terminal = Terminal::new(TestBackend::new(180, 24)).expect("terminal");
    // Before preview: hint absent.
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(!rendered.contains("w: word wrap"), "hint absent before preview opens");
    // After preview: hint present.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let rendered = buffer_text(terminal.backend().buffer());
    assert!(rendered.contains("w: word wrap"), "hint visible when preview open");
}
```

**Evidence command:** `cargo test -q -- word_wrap 2>&1`

---

### Execution order and verification

| Step | File | Verify |
|------|------|--------|
| 1 | src/app.rs — field + accessor + toggle + reset | `cargo test -q` |
| 2 | src/app.rs — key handler | `cargo test -q` |
| 3 | src/ui/mod.rs — conditional wrap render | `cargo test -q` |
| 4 | src/ui/mod.rs — footer hint | `cargo test -q` |
| 5 | src/ui/mod.rs — help popup hint | `cargo test -q` |
| 6 | src/ui/mod.rs — three new tests | `cargo test -q -- word_wrap` |

Final verification after all steps: `cargo test -q 2>&1 | tail -10` — all tests pass, zero warnings from new code.

### Module and file ownership notes

- `src/app.rs` owns all state mutations; `OverviewState` struct, `reset_preview_scroll`, and the `AppMode::Overview` key handler are the three sub-sites.
- `src/ui/mod.rs` owns all rendering; `render_preview`, `render_status_footer`, and `render_help_popup` are the three sub-sites.
- No cross-crate changes; `Wrap` is already imported in `src/ui/mod.rs` (line 12).
- Tests live in the `#[cfg(test)]` block at the bottom of `src/ui/mod.rs` alongside existing integration tests.

## Stage Report: plan

- DONE: Plan covers all five touch points: OverviewState field, toggle handler, reset_preview_scroll, render_preview conditional wrap, help footer and popup legend update.
  Each touch point maps to a numbered step (Steps 1–5) with exact file, line range, code diff, and `cargo test -q` evidence command.
- DONE: Plan includes snapshot/unit test strategy for AC-1 (toggle re-renders), AC-2 (reset on pane switch), AC-3 (legend visible).
  Step 6 defines three named test functions, one per AC, with full code and evidence command `cargo test -q -- word_wrap`.

### Summary

The plan breaks the feature into six sequential steps: adding the `preview_wrap` bool field and supporting methods to `OverviewState` (Step 1), wiring the `w` key handler in the `AppMode::Overview` match arm (Step 2), applying conditional `.wrap()` in `render_preview()` with `max_preview_scroll_x` clamped to 0 (Step 3), adding the footer hint (Step 4), adding the help popup line (Step 5), and writing three test functions covering all three acceptance criteria (Step 6). Each step specifies exact line locations, minimal code changes, and a `cargo test -q` verification command. No new dependencies are needed.
