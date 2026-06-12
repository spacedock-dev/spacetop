---
id: "057"
title: Mouse support for Spacetop panels
status: plan
source: captain request 2026-06-12
kind: feature
risk: medium
milestone: v2-p6
proof:
started: 2026-06-12T06:36:10Z
completed:
verdict:
score: 0.7
worktree:
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
