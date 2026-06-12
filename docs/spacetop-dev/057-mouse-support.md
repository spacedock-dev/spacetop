---
id: "057"
title: Mouse support for Spacetop panels
status: implement
source: captain request 2026-06-12
kind: feature
risk: medium
milestone: v2-p6
proof:
started: 2026-06-12T06:36:10Z
completed:
verdict:
score: 0.7
worktree: .worktrees/spacedock-ensign-057-mouse-support
issue:
pr:
---

Add mouse interaction to the Spacetop TUI so the captain can inspect Spacedock
workflow state without keyboard round-trips. Captain-confirmed behaviors:

1. **Click selects + opens preview** — a single left-click on an entity row in
   the overview list selects it and immediately opens/updates the preview
   sub-panel.
2. **Scroll wheel scrolls the preview** — when the preview panel is open, the
   mouse wheel scrolls its content (and the entity list when hovering it).
3. **Drag-divider resize** — click and drag the border between the list and
   the preview panel to resize the split, like a GUI splitter.
4. **Native text selection stays available via Shift+drag** — the app keeps
   mouse capture, relying on the standard terminal convention that Shift+
   left-drag bypasses capture so the terminal handles selection/copy natively
   (iTerm2, Terminal.app, kitty, WezTerm). No plain-drag passthrough is
   expected; this convention must be documented in the UI help surface.

Captain-selected scope: overview entity list, preview sub-panel, workflow
picker (click to choose a workflow), and stage graph (click a stage node).

**Shape decision — stage graph splits out.** The graph renderer
(`crates/spacetop/src/ui/graph.rs`) is a pure paint function with three
width tiers (wide ribbon, wrapped narrow, very-narrow grid) that compute
node positions internally during rendering. Click support requires exposing
a node-to-rect map across all three tiers — a renderer restructure that
would dominate this task — and the click *semantics* (select stage? filter
list by stage?) are themselves unconfirmed product behavior. Per the
captain's pre-authorization, stage-graph click is split out as a follow-up
entity to be filed when this task lands; it is a non-goal here.

Problem statement: inspecting workflow state in Spacetop today requires
keyboard round-trips for selection, preview, and panel sizing. The target
outcome is that a captain can drive the core inspection loop — pick a
workflow, pick an entity, read its preview at a comfortable split — with
the mouse alone, without losing native terminal copy/paste.

## Scope

- Kind: feature
- Risk: medium — touches the terminal lifecycle (mouse capture must be
  released on exit and on panic or the captain's terminal is left eating
  clicks), the event loop, and rendering geometry; no parser, git, or
  watcher changes.
- Milestone: v2-p6 (continues the v2 phase sequence; first post-rebuild UX
  phase)
- Touches:
  - app-state: `crates/spacetop/src/app/` — new typed mouse-event handling
    (a `handle_mouse` peer to `handle_key`), hit-testing of rows/divider,
    divider split ratio as `OverviewState` fact. Hit-testing needs the
    last-rendered panel rects; the repo precedent is `Cell`-based render
    facts (`max_preview_scroll`), so rects recorded at render time or a
    pure shared layout function are both admissible — plan decides.
  - event loop / lifecycle: `crates/spacetop/src/lib.rs` `run_terminal` —
    `EnableMouseCapture` after `EnterAlternateScreen`, `DisableMouseCapture`
    in `TerminalRestore` (covers both normal exit and panic/early-return via
    the existing drop guard), `Event::Mouse` arm beside `Event::Key`.
  - ui rendering: `crates/spacetop/src/ui/mod.rs` + `ui/layout.rs` — the
    fixed `Percentage(50/50)` / `Percentage(30/70)` list/preview constraints
    become driven by the state-held split ratio; `ui/picker.rs` row
    geometry exposed for hit-testing.
  - docs: `ui/help.rs` help popup (Shift+drag convention), `ui/footer.rs`
    hints if room permits, README key documentation.
- Non-goals:
  - No mutation of Spacedock workflow files — mouse support adds zero
    writes; the `no_write_git_calls` guardrail and read-only contract are
    untouched.
  - No stage-graph node hit-testing (split out, above).
  - No plain-drag text-selection passthrough and no mouse-capture toggle
    key (captain-confirmed: Shift+drag is the mechanism).
  - No custom per-terminal mouse protocols beyond crossterm's standard
    `EnableMouseCapture`.
  - No double-click, right-click, or context-menu semantics.
  - No persistence of the divider ratio across sessions (in-memory per
    session is sufficient; plan may add session-state persistence only if
    trivial).

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 — Click selects and opens the preview.**
A single left-click on an entity row in the overview list selects that row
and opens (or updates) the preview sub-panel in one action. Clicks on
non-row chrome (borders, blank space) change nothing.
Verified by: app-state unit tests mapping a typed mouse-down at row
coordinates to selection + `preview_open` transitions, no terminal backend.

**AC-2 — Wheel scrolls the panel under the cursor.**
With the preview open, `ScrollUp`/`ScrollDown` over the preview moves
`preview_scroll` (clamped to the existing max); the same events over the
entity list move the list selection/offset. Hover position decides the
target.
Verified by: app-state unit tests with scroll events at preview vs list
coordinates.

**AC-3 — Dragging the divider resizes the split.**
Mouse-down on the list/preview divider followed by drag moves the split
ratio continuously, clamped to minimum usable sizes for both panels, in
both Left and Bottom preview placements. The ratio holds until changed
again within the session.
Verified by: pure hit-test tests for the divider band plus drag-sequence
state tests (down → drag → up) in both placements.

**AC-4 — Native text selection stays available and is documented.**
The app holds mouse capture for its lifetime, and Shift+left-drag reaches
the terminal for native selection/copy (standard iTerm2 / Terminal.app /
kitty / WezTerm convention). The convention is stated in the help popup
(`?`).
Verified by: help-popup content test pinning the Shift+drag line, plus a
real-terminal manual check that Shift+drag selects text while the app runs.

**AC-5 — Click chooses a workflow in the picker.**
A single left-click on a workflow row in the picker (and picker overlay)
selects and confirms it — same one-action convention as AC-1.
Verified by: app-state unit tests mapping a click at picker-row coordinates
to the confirm transition.

**AC-6 — Mouse capture lifecycle is safe.**
`EnableMouseCapture` is issued at startup and `DisableMouseCapture` is
restored on normal exit AND on panic/early error, so no exit path leaves
the terminal swallowing mouse input. This is a product-contract surface,
not an implementation detail.
Verified by: extending the existing fake-backend lifecycle-order tests in
`lib.rs` (the `TerminalRestore` drop-guard log) to assert capture
enable/disable ordering on both paths.

## Proof plan

- Lowest test layer: pure hit-test and app-state unit tests (typed mouse
  event at coordinates → state transition) with no terminal backend;
  lifecycle ordering via the existing fake-backend log in `lib.rs`.
- Required command: `cargo test --workspace`; `make lint`.
- Manual check, if any: real-terminal smoke (iTerm2 or Terminal.app) —
  click-select, wheel scroll, divider drag feel, Shift+drag native
  selection, and clean capture release on quit and on Ctrl+C.
- Docs/policy update needed: help popup + README mouse/Shift+drag note;
  development policy untouched (read-only contract unchanged).

## Stage Report: shape

- DONE: Formalize the four captain-confirmed mouse behaviors into acceptance criteria as end-state properties, each with a named verification path
  AC-1..AC-5 written (click-select+preview, hover-targeted wheel scroll, divider drag in both placements, Shift+drag + help documentation, picker click); each names its test layer.
- DONE: Set scope boundaries and non-goals: decide whether stage-graph click interaction stays in this task minimally or splits into a follow-up; name the mouse-capture lifecycle (enable on start, restore terminal on exit AND on panic) as a product-contract surface; confirm the read-only Spacedock-file contract is untouched by mouse support
  Stage graph SPLIT OUT (renderer computes node positions internally across 3 width tiers; hit-test map would dominate the task and click semantics are unconfirmed); capture lifecycle is AC-6 riding the existing TerminalRestore drop guard; read-only contract named in non-goals with the no_write_git_calls guardrail.
- DONE: Fill frontmatter risk, milestone, and score, and name the modules touched (app-state vs ui rendering vs docs) so plan can separate event-handling work from rendering work
  risk: medium, milestone: v2-p6, score: 0.7; Touches section separates app-state (handle_mouse, hit-testing, split-ratio fact), lifecycle (lib.rs run_terminal), ui rendering (mod.rs/layout.rs/picker.rs), and docs (help.rs, footer.rs, README).

### Summary

Shaped mouse support into six end-state ACs grounded in the actual code: fixed Percentage constraints in ui/mod.rs become a state-held split ratio, hit-testing follows the existing Cell render-fact precedent, and capture release rides the TerminalRestore drop guard so panic paths are covered. Stage-graph click is recommended as a follow-up entity rather than a minimal inclusion, since its renderer restructure and unconfirmed semantics would bloat an otherwise well-bounded task.

## Implementation Plan

### Geometry decision: render-recorded rects are the single source of truth

Hit-testing consumes only rects (and the list scroll offset) that the render
pass records into `Cell` render-facts on app state — the same values the
widgets were drawn with, in the same frame. Click coordinates therefore
cannot drift from drawn rows **by construction**. A pure shared layout
function alone cannot be the source of truth because the overview list's
scroll offset is computed inside ratatui's `List` widget during
`render_stateful_widget`; it is only observable afterwards via
`ListState::offset()` (ratatui 0.30). Freshness is guaranteed by the
existing event-loop invariant — `run_terminal` draws before it polls input —
already documented and relied on by `preview_viewport_height` in
`app/overview.rs`. The picker has the same shape already
(`viewport_height`/`scroll_offset` Cells in `app/picker.rs`).

A pure function still owns the *split computation*:
`split_content(content: Rect, placement, list_percent) -> (list, preview)`
in `ui/layout.rs`, unit-testable for clamping with no backend. Render calls
it; recorded rects remain what hit-testing reads.

New render-facts on `OverviewState` (all `Cell<_>`, mirroring
`max_preview_scroll`): `content_rect: Cell<Rect>`, `list_rows_rect:
Cell<Rect>` (rows area, after the 1-row section header), `list_offset:
Cell<usize>` (from `ListState::offset()` after render), `preview_rect:
Cell<Rect>` (reset to `Rect::default()` when the preview is closed so wheel
events never hit a stale rect). `ratatui::layout::Rect` is `Copy + Default +
PartialEq`, so the existing derives on `OverviewState` hold. `PickerState`
gains `list_rect: Cell<Rect>`.

The divider is the preview block's own border (`Borders::LEFT` in Left
placement, `Borders::TOP` in Bottom — `ui/preview.rs`), so the divider band
is derived from `preview_rect`: the border column/row widened by 1 cell each
side for grabbability.

### Step 1 — Capture lifecycle + event plumbing (AC-6)

Owned files: `crates/spacetop/src/lib.rs`, `crates/spacetop/src/app.rs`
(no-op `handle_mouse` stub).

- Startup: `execute!(stdout, EnterAlternateScreen, EnableMouseCapture)`.
- Extend the `TerminalControl` trait with `enable_mouse_capture` /
  `disable_mouse_capture`; implement on `CrosstermTerminalControl`.
- Refactor the `TerminalRestore::drop` body into a testable free function
  `restore_terminal<T: TerminalControl>` (disable raw mode, leave alt
  screen, disable mouse capture); `Drop` calls it with the crossterm impl,
  covering normal exit, panic, and early `?`-return via the existing guard.
- $EDITOR suspend/resume (`suspend_terminal`/`resume_terminal`) releases
  capture on suspend and re-enables on resume, so the editor's terminal
  does not eat clicks.
- Event loop: replace the `if let Event::Key` read with a match that adds
  `Event::Mouse(m) => app.handle_mouse(m)`.
- Contingency (manual probe, below): if divider drag feels laggy at one
  event per 100ms tick, drain pending events with an inner
  `while event::poll(Duration::ZERO)` loop — not done up front.

Tests (existing fake-backend seam, no terminal): extend
`suspend_resume_call_sequence` to pin capture off/on ordering; new
`terminal_restore_sequence` test pinning
`disable_raw_mode → leave_alt → disable_mouse_capture` on the
`MockTerminalControl` log.

Manual probe gate (do immediately after this step, before any hit-testing
work): run `cargo run -p spacetop -- -w docs/spacetop-dev` in iTerm2 and
Terminal.app; verify Shift+left-drag selects text natively while capture is
on, clicks stop being swallowed after `q`, and the $EDITOR round-trip (`o`)
leaves selection working inside the editor. This is the one
terminal-emulator-dependent unknown that no test layer can prove; it gates
the rest of the task and needs no throwaway spike code.

### Step 2 — Split-ratio state + shared geometry facts (groundwork for AC-1/2/3)

Owned files: `crates/spacetop/src/app/overview.rs`,
`crates/spacetop/src/ui/layout.rs`, `crates/spacetop/src/ui/mod.rs`,
`crates/spacetop/src/ui/list.rs`.

- App-state: move `PreviewPlacement` from `ui/layout.rs` into
  `app::overview` (ui already imports from `crate::app`; app must stay
  backend-free, and layout types are pure). Add `split_percent_left: u16`
  (default 50) and `split_percent_bottom: u16` (default 30) — one per
  placement so the current defaults are preserved and each placement holds
  its own ratio for the session (per `OverviewState`, i.e. per tab; no
  persistence per non-goals). Add `divider_drag: bool` (plain field) and the
  four render-fact Cells listed above.
- `ui/layout.rs`: `split_content(content, placement, list_percent)`
  computes the two rects directly (not via `Layout`) so min-size clamping
  is explicit: clamp so both panes keep a minimum usable size (~10 cols /
  3 rows; exact constants pinned by tests).
- `ui/mod.rs` `render_overview`: replace the fixed
  `Percentage(50/50)`/`Percentage(30/70)` constraints with
  `split_content(...)` driven by state; record `content_rect` and
  `preview_rect` (or reset `preview_rect` when closed).
- `ui/list.rs` `render_task_list`: record `list_rows_rect` (the
  header-shifted rows area) and, after `render_stateful_widget`,
  `list_offset` from `ListState::offset()`.

Tests: pure `split_content` unit tests (default ratios reproduce today's
50/50 and 30/70; clamping at extreme percents and tiny areas). One
TestBackend render test tying fact to pixels: render a known snapshot,
then assert the buffer shows entity N's title exactly at
`list_rows_rect.y + (N - list_offset)` — the anti-drift proof.

### Step 3 — handle_mouse + overview hit-testing (AC-1, AC-2, AC-3)

Owned files: new `crates/spacetop/src/app/mouse.rs`,
`crates/spacetop/src/app.rs` (mode dispatch),
`crates/spacetop/src/app/overview.rs` (small mutators).

- `app/mouse.rs`: `enum OverviewHit { ListRow(usize), Divider, Preview,
  Chrome }` and pure `fn overview_hit(&OverviewState, column, row) ->
  OverviewHit` reading only the render-facts. Row index =
  `list_offset + (row - list_rows_rect.y)`, valid only when `<
  row_count()` (covers synthetic broken rows).
- `handle_overview_mouse(session, MouseEvent) -> OverviewKeyAction` (peer
  to `handle_overview_key_with_keymap`; reuses the action enum, returning
  `None` for all current cases):
  - `Down(Left)` on `ListRow(i)` → select row `i` and open the preview if
    closed (new `open_preview()`/`select_row(i)` mutators on
    `OverviewState`; selection change reuses the existing
    `set_scope_index` reset semantics). One action: select + open (AC-1).
  - `Down(Left)` on `Divider` → `divider_drag = true`;
    `Drag(Left)` while dragging → recompute the active placement's split
    percent from the cursor position relative to `content_rect`, clamped
    (10..=90 plus `split_content` min sizes); `Up(_)` → end drag (AC-3).
  - `ScrollUp`/`ScrollDown` over `Preview` → clamped wheel scroll, 3 rows
    per notch (new `wheel_scroll_preview(delta)` wrapping the existing
    clamped `scroll_preview_vertical`); over `ListRow`/list area →
    `select_previous()`/`select_next()` (AC-2; hover position decides).
  - Everything else (`Chrome`, non-left buttons, drags without a divider
    grab) → no state change.
- `App::handle_mouse` in `app.rs`: no-op while `help_open`; dispatches
  `AppMode::Overview` to `handle_overview_mouse`; Picker modes in Step 4;
  all other modes (Definition/Search/Timeline/Metrics/Activity/Relations)
  deliberately inert per the captain-selected scope.

Tests (app-state layer, no real terminal): a fixture renders a known
snapshot once through `TestBackend` to populate the facts, then drives
typed `crossterm::event::MouseEvent`s. AC-1: click at row-2 coords selects
index 2 and sets `preview_open`; click on the graph ribbon/blank area
changes nothing. AC-2: scroll events at preview coords move
`preview_scroll` (clamped at max); at list coords move the selection.
AC-3: down→drag→up sequences on the divider in a wide (Left, e.g.
100x30) and a tall (Bottom, e.g. 60x40) backend mutate the right percent
field, clamp at the edges, and a re-render honors the new ratio; the ratio
survives until changed again.

### Step 4 — Picker click (AC-5)

Owned files: `crates/spacetop/src/app/picker.rs`,
`crates/spacetop/src/ui/picker.rs`, `crates/spacetop/src/app.rs`,
`crates/spacetop/src/app/mouse.rs`.

- `ui/picker.rs` `render_list`: record `list_rect` into the new
  `PickerState` Cell (it already records `viewport_height` and
  `scroll_offset`, which double as the hit-test offset).
- Refactor the `PickerOverlay` Enter-confirm body in `App::handle_key`
  into `App::confirm_picker_overlay()` so keyboard and mouse share one
  transition (standalone picker already shares `picker_enter_transition`).
- `App::handle_mouse`: `Down(Left)` inside `list_rect` → set
  `selected_index = scroll_offset + (row - list_rect.y)` when in range,
  then confirm via the shared transition — one action, mirroring AC-1.
  Clicks elsewhere in the dialog: nothing.

Tests: tempdir workflows (existing `write_minimal_workflow` precedent in
`lib.rs` tests), render the picker via TestBackend to populate facts, click
a row: standalone picker transitions to `AppMode::Overview` rooted at the
clicked workflow; overlay click sets `pending_switch` to the clicked index.
Click on the title/footer rows changes nothing.

### Step 5 — Documentation surfaces (AC-4)

Owned files: `crates/spacetop/src/ui/help.rs`,
`crates/spacetop/src/ui/tests/chrome.rs`, `README.md`.

- `ui/help.rs`: a short mouse block — click select/open, wheel scroll,
  drag divider resize, and the load-bearing line: Shift+drag for native
  terminal text selection (iTerm2 / Terminal.app / kitty / WezTerm
  convention).
- README: a "Mouse" subsection documenting the same four behaviors and the
  Shift+drag convention.
- Footer (`ui/footer.rs`): deliberately unchanged. The single status line
  already runs ~80 cols; the Touches note says "if room permits" and it
  does not. Mouse affordances are self-discoverable; the help popup is the
  documented surface AC-4 names.

Tests: chrome.rs help-popup test pinning the Shift+drag line (stable
user-facing string; pin string and test together per repo convention).

### Step 6 — Verification gate

- `cargo test --workspace` — all layers above.
- `make lint` — completion gate; clippy `-D warnings`, no new `#[allow]`.
- Manual real-terminal smoke (iTerm2 + Terminal.app): click-select,
  wheel over both panels, divider drag feel in both placements (resize the
  window across the aspect threshold), Shift+drag native selection, clean
  capture release on `q` and on Ctrl+C, $EDITOR round-trip.

### Per-AC verification map

| AC | Lowest layer | Command |
|----|--------------|---------|
| AC-1 click select+preview | app-state unit (`app/mouse.rs` tests; TestBackend populates facts, typed `MouseEvent` drives) | `cargo test -p spacetop mouse` |
| AC-2 hover-targeted wheel | same layer, scroll events at preview vs list coords | `cargo test -p spacetop mouse` |
| AC-3 divider drag | pure `split_content` clamp tests (`ui/layout.rs`) + drag-sequence state tests, both placements | `cargo test -p spacetop layout mouse` |
| AC-4 Shift+drag documented | help-popup buffer test pinning the line | `cargo test -p spacetop chrome` + manual real-terminal check |
| AC-5 picker click confirm | app-state tests over tempdir workflows | `cargo test -p spacetop` |
| AC-6 capture lifecycle | `TerminalControl` mock-log ordering tests in `lib.rs` (startup, restore, suspend/resume) | `cargo test -p spacetop --lib` + manual quit/Ctrl+C check |
| Gate | whole workspace | `cargo test --workspace && make lint` |

"Without a real terminal" means: ratatui `TestBackend` (an in-memory
buffer) populates the same render-facts the production draw does; mouse
events are plain `crossterm::event::MouseEvent` structs constructed in
tests; no PTY, raw mode, or capture is involved anywhere in the proof.

### Spikes, fixtures, and manual probes

- **No code spike needed.** The one genuine unknown — whether Shift+drag
  passthrough and capture release behave per convention in real emulators —
  is a 10-minute manual probe gated at the end of Step 1, before any
  hit-testing investment. It cannot be automated: it tests
  terminal-emulator behavior, not app behavior.
- **Fixture:** the existing `MockTerminalControl` log is extended for AC-6;
  a small render-then-click test fixture (snapshot + TestBackend draw)
  serves Steps 2-4. Nothing machine-specific; all in-repo.
- **Geometry source of truth:** resolved above — render-recorded facts,
  not a parallel computation; the Step 2 anti-drift test ties the recorded
  rect to the drawn buffer.

### Policy notes

- Read-only contract untouched: mouse handling mutates in-memory state
  only; zero new process spawns or file writes, so the
  `no_write_git_calls` guardrail is unaffected.
- Clean Code: keyboard/mouse share the `OverviewKeyAction` application path
  and the picker confirm transition rather than duplicating transitions;
  scroll clamping stays in the single existing `scroll_preview_vertical`
  home.

## Stage Report: plan

- DONE: Produce a step-by-step implementation plan that separates app-state/event-handling work (a handle_mouse peer to handle_key, row/divider hit-testing, a state-held split ratio replacing fixed Percentage constraints) from rendering work and from the capture lifecycle in run_terminal, naming the owned files for each step.
  Six steps with owned files each: Step 1 lifecycle (lib.rs), Step 2 split state + geometry facts (app/overview.rs, ui/layout.rs, ui/mod.rs, ui/list.rs), Step 3 handle_mouse/hit-test (new app/mouse.rs, app.rs), Step 4 picker (app/picker.rs, ui/picker.rs, app.rs), Step 5 docs (ui/help.rs, README.md), Step 6 gate.
- DONE: Name the exact verification command and lowest practical test layer for each of AC-1 through AC-6, including how hit-testing and the split-ratio fact are proven without a real terminal, and include make lint as the completion gate.
  Per-AC verification map table: app-state unit tests with TestBackend-populated render facts and typed MouseEvent structs (AC-1/2/3/5), pinned help-popup string (AC-4), MockTerminalControl ordering log (AC-6); gate is cargo test --workspace && make lint.
- DONE: Identify any spike or fixture needed before risky work -- in particular whether the layout/hit-test geometry the app computes can be made the single source of truth shared with rendering (so click coordinates cannot drift from drawn rows), and what, if anything, about mouse-capture lifecycle or Shift+drag passthrough needs a manual probe rather than a test.
  Geometry: render-recorded Cell facts (incl. ListState::offset()) are the single source of truth — a pure shared function cannot capture ratatui's internal list offset; freshness rides the documented draw-before-poll loop invariant. No code spike; one manual probe (Shift+drag passthrough + capture release in iTerm2/Terminal.app) gated at end of Step 1 before hit-testing work, since it tests emulator behavior no test layer can reach.

### Summary

Planned mouse support as six steps that keep app-state, rendering, and terminal-lifecycle work in separate commits, with the capture-lifecycle step first so the only untestable unknown (Shift+drag passthrough in real emulators) is probed manually before the hit-testing investment. The load-bearing design decision is that hit-testing consumes only rects and list offsets recorded by the render pass itself (Cell render-facts, the repo's existing precedent), making coordinate drift impossible by construction, while a pure split_content function owns ratio clamping so AC-3 is provable without any terminal. Every AC has a named command and lowest test layer; make lint is the completion gate; the read-only contract is untouched.
