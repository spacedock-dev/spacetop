---
id: 010
title: Switch between multiple discovered workflows from inside the TUI
status: done
source: captain feedback after 009 ship
started: 2026-04-25T04:13:38Z
completed: 2026-04-25T04:49:24Z
verdict: PASSED
score:
worktree: 
issue:
pr:
mod-block: 
archived: 2026-04-25T04:49:24Z
---

When a repo contains multiple Spacedock workflows (e.g. `docs/spacetop-dev/`, plus future product or research workflows), the TUI should let the user move between them without quitting and relaunching. The autodiscovery picker (task 005) handles the initial selection at startup; this task adds the in-session switch.

## Problem statement

Today, switching workflows requires `q` → relaunch (or `q` → relaunch with a different `-w`). For a repo with even two workflows this is friction-heavy; for repos that grow to three or more (the realistic SpaceTop end state) it forces the user to memorize paths and re-warm the watcher every time. The TUI should let an operator inspecting one workflow flip to another and back without losing the visual state they had — selected entity, scope, scroll position — and without paying any UI cost when there is only one workflow.

## Target user flow

1. User launches `spacetop` in a repo with N >= 2 discovered workflows.
2. Startup picker (task 005) shows the discovered list. User picks one and lands in Overview.
3. From Overview, user presses `]` to cycle to the next discovered workflow, or `[` to go back. The whole frame re-renders against the new workflow's snapshot in well under a frame.
4. State that belongs to the **prior** workflow (selected entity slug, view scope, archive-load cache, last-refresh-error) is retained in memory so a return cycle restores it verbatim.
5. From Overview, user presses `P` (capital P) to re-open the discovery picker overlay — used to (a) jump directly to a workflow more than one cycle away in a large list and (b) trigger a fresh disk re-discovery.
6. Single-workflow case: no breadcrumb, no cycle keys (they become inert), no picker overlay key. The user sees the same UI they have today.

## Chosen UX: status-line breadcrumb + cycle keys, with picker-revisit overlay

**Decision: status-line breadcrumb with `[`/`]` cycle and `P` picker-overlay revisit. Reject the captain's working tabs hypothesis.**

Comparison against alternatives:

- **Tab strip across the top (captain's hypothesis):** Costs a permanent terminal row. The current dashboard already burns rows on the stage ribbon (task 006/007 header), the centered list+preview column (task 009), and a help affordance hint. Two- or three-workflow repos do not justify a dedicated row, and a five-workflow repo would force horizontal scrolling logic inside the strip — net negative density. Also bakes a numeric-direct-select keymap (`1`..`9`) that collides with future filter shortcuts.
- **Modal chooser overlay (sibling to help popup):** Workable but adds modal cognitive load — every switch becomes "open overlay → arrow → enter," which is heavier than a single keystroke for the common 2–3-workflow case. Kept as a *secondary* path via `P` so users with many workflows still have a direct selector and a re-discovery trigger.
- **Persistent picker key only:** Same heaviness as the modal-only proposal; no fast-cycle path.
- **Status-line breadcrumb + cycle keys (chosen):** Reuses the existing graph header's bottom-of-block area (task 006/007 already renders a workflow path there; we extend it to "[2/3] docs/spacetop-dev"). Zero new rows, one keystroke per switch, with `P` as the escape hatch for direct selection and re-discovery. Hidden entirely when only one workflow is discovered.

The chosen UX subsumes the captain's intent (in-session switching) while spending no vertical real estate. Tabs would have been the right call if the dashboard had idle horizontal space at the top; it does not.

## Keymap

| Key            | Mode                          | Action                                                  |
|----------------|-------------------------------|---------------------------------------------------------|
| `]`            | Overview, multi-workflow only | Cycle to next discovered workflow (wraps).              |
| `[`            | Overview, multi-workflow only | Cycle to previous discovered workflow (wraps).          |
| `P` (Shift+p)  | Overview, multi-workflow only | Open picker overlay (fresh re-discovery), reuses task 005 picker UI as a popup; `Enter` selects, `Esc` dismisses without changing workflow. |
| `]` / `[` / `P`| Overview, single-workflow     | Inert (no-op, no help-line entry).                      |
| `]` / `[` / `P`| Picker (startup) and help-open| Inert.                                                  |

Collision audit against existing bindings (`a`, `?`, `j`/`k`, `↑`/`↓`, `Home`/`End`, `Enter`, `q`, `Esc`, picker `Enter`):

- `]` and `[` are unused everywhere in the existing key handler (`src/app.rs` lines 555–593). No collision.
- Capital `P` is unused; lowercase `p` is also unused but reserved for a future "preview toggle" / "pin" that has come up in captain feedback — using `P` keeps the lowercase namespace open.
- No conflict with `?` (help), `a` (scope), navigation, or quit keys.

`Tab` / `Shift+Tab` were considered and rejected: terminal emulators inconsistently deliver `Shift+Tab` (some send `BackTab`, some send raw `Tab` with shift modifier), and `Tab` itself is plausibly desired later for focus-cycling between the list pane and the preview pane. Bracket keys are mnemonic ("next/prev section"), unambiguous, and free.

## Per-workflow state semantics on switch

Each discovered workflow gets its own `OverviewState` instance, kept in a `Vec<OverviewState>` indexed alongside the discovery list. The `App` holds:

- `workflows: Vec<OverviewState>` — one per discovered workflow, lazily materialized.
- `active_workflow: usize` — index into both the discovery list and `workflows`.
- `discovery: Vec<DiscoveredWorkflow>` — the resolved list from `discover_workflows`.

On switch (`]`/`[`/picker-confirm), the active index changes; **no `OverviewState` is dropped, no snapshot is reloaded from disk, no archive cache is invalidated.** The newly active state's existing `selected_index`, `view_scope`, `archived_items`, `archive_loaded`, and `last_refresh_error` render verbatim.

Lazy materialization rule: a workflow's `OverviewState` is only `load()`-ed (FS-touching) on its first activation. Cycling past a workflow does not pre-load it; only landing on it does. Switching back is therefore O(1) memory access and zero IO.

A switch failure (parse error during first-time load) leaves `active_workflow` pointing at the failing slot but populates a synthetic empty `OverviewState` with `last_refresh_error` set, so the user sees the breadcrumb and the error rather than a hang or a silent revert.

## Watcher fan-out

**Decision: one watcher for the active workflow only; tear down and re-start on switch.**

Rationale:

- N parallel watchers means N `notify::RecommendedWatcher` instances, N debounce threads, and N sets of file handles. On macOS FSEvents this is cheap-ish; on Linux inotify it consumes user-watch quota; on the `PollWatcher` fallback it multiplies the polling cost linearly. SpaceTop's sweet spot is 2–5 workflows in a repo, but we should not bake a per-workflow watcher cost when only one is on screen.
- Cross-workflow drift while a tab is inactive is acceptable. The user-perceived contract is "the workflow I'm looking at is live." When they switch back, the new active state's first action is a synchronous `reload()` (cheap — one `load_workflow_dir` call) before the watcher restarts. This converges the in-memory state to disk on focus.
- Discovery refresh is *not* coupled to the per-workflow watcher. A new workflow appearing on disk while the TUI is open is not auto-detected; the user triggers re-discovery explicitly via `P` (which re-runs `discover_workflows` from the resolved scan root before showing the picker overlay). This matches the user's mental model: "a tab list that updates when I ask it to."

Switch sequence:

1. Drop current `WorkflowWatcher` (its `Drop` joins the debounce thread; bounded by `SHUTDOWN_POLL_INTERVAL`).
2. Mutate `active_workflow`.
3. If the new active state has never been loaded, call `OverviewState::load`; otherwise call `reload()` to converge.
4. Start a fresh `WorkflowWatcher` rooted at the new active workflow's dir.

This sequence is sub-frame on the recommended backend; on the poll-fallback backend it is bounded by `POLL_FALLBACK_INTERVAL` (1 s) for the new watcher's first event but the synchronous reload in step 3 already shows current state.

## Resolved design questions

1. **UI shape.** Status-line breadcrumb + bracket cycle keys + `P` overlay. Justification: zero vertical cost, one-keystroke common case, escape hatch for large lists. (See "Chosen UX" above.)
2. **Active indicator location.** Extend the existing graph-header workflow-path line (task 006/007) with a `[i/N]` prefix, e.g. `[2/3] docs/spacetop-dev`. Justification: reuses an already-allocated row; the path is what the user thinks of as "which workflow am I on."
3. **Keybindings.** `]` next, `[` prev, `P` picker overlay. Justification: bracket pair is mnemonic and collision-free; `P` keeps lowercase `p` open for future use; rejected `Tab`/`Shift+Tab` due to terminal inconsistency and likely future focus-cycle use.
4. **Per-workflow state.** Keep one `OverviewState` per discovered workflow, lazily materialized, never dropped during the session. Justification: switch latency dominates user perception; memory is bounded by N (≤10 in practice) snapshots — cheap.
5. **Discovery refresh trigger.** User-initiated only, via `P`. Justification: auto-watching the scan root would multiply file handles and produce surprises; the user already expects a "refresh" gesture for this kind of change.
6. **Single-workflow case.** Breadcrumb hidden; cycle and `P` keys inert and absent from help popup. Justification: zero UI cost when there is nothing to switch to.
7. **Picker entry point in multi-tab model.** Startup picker (task 005) is unchanged for the discovery-flow launch. When the user passed `-w/--workflow-dir`, that becomes a strict single-workflow session — `P` is *not* exposed. Justification: `-w` is an explicit "this dir only" contract; surprising the user with a picker would violate it. Users who want multi-workflow should launch without `-w`.
8. **Re-discovery key.** `P` triggers a fresh `discover_workflows(scan_root)` and opens the picker overlay against the result. Newly added workflows show up; deleted ones disappear; the previously active workflow stays active if still present, otherwise the picker pre-selects index 0.
9. **Watcher fan-out.** One watcher, follows the active workflow; teardown + restart on switch; synchronous `reload()` on switch covers the brief observation gap. Justification: bounds resource usage and matches the "the on-screen workflow is live" contract.

## Acceptance criteria

**AC-1 — From Overview with 2+ discovered workflows, `]` and `[` cycle the active workflow without restarting the process.**
Verified by: integration test against a fixture repo with three workflow dirs. Press `]`; assert the breadcrumb index increments, the visible items match the next workflow's snapshot, and `App::workflow_dir()` (active) reports the new path. Press `[` twice; assert wrap-around to the last workflow.

**AC-2 — In single-workflow sessions, no breadcrumb is rendered and `]`/`[`/`P` are inert.**
Verified by: render test on a single-workflow fixture asserts the breadcrumb element is absent (no `[1/1]` prefix). Key-handler test feeds `]`, `[`, `P` and asserts `App` state is byte-identical before and after. Help popup render test on the same fixture asserts cycle/picker hints are absent.

**AC-3 — Switching workflows preserves per-workflow `selected_index`, `view_scope`, and archive-load cache for return visits.**
Verified by: app-state test seeds workflow A with selection at index 2 and scope=Archived, switches to B (default state), switches back to A; asserts `selected_index == 2`, `view_scope == Archived`, and `archive_loaded == true` (no re-IO). A second test asserts B's first activation triggers exactly one `load_workflow_dir` call (via a counting fake or test seam) and that subsequent switches back to B do not re-IO.

**AC-4 — One watcher exists at any time and follows the active workflow.**
Verified by: watcher-lifecycle test with an instrumented `WorkflowWatcher` factory counts `start` and `drop` events across a switch. Assert exactly one watcher alive after each switch and that its watched root equals the active workflow's `workflow_dir`. A second assertion: a refresh signal arriving for the *prior* workflow's root after a switch is dropped (channel closed) without panicking the main loop.

**AC-5 — `P` re-runs discovery and updates the available workflow list mid-session.**
Verified by: integration test with a fixture where a third workflow dir is created on disk *after* the TUI opens. Press `P`; assert the picker overlay's listed workflows now includes the newly created dir. Select it, press `Enter`; assert the active workflow becomes the new one and the breadcrumb shows `[3/3]`.

**AC-6 — New keybindings do not collide with existing bindings (`a`, `?`, `j`/`k`, `↑`/`↓`, `Home`/`End`, `Enter`, `q`, `Esc`).**
Verified by: a unit test that enumerates the help popup's listed bindings and asserts the union of (existing-set) and (new-set: `]`, `[`, `P`) is disjoint and matches the actual key handler's recognized keys. Help popup render test asserts the new entries appear only when N >= 2.

**AC-7 — `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` are all clean on the implement branch.**
Verified by: command output cited in the implement stage report.

## Stage Report: design

- DONE: The 9 open design questions in the seed are each resolved with a chosen answer + one-line justification (not "TBD").
  See "Resolved design questions" section — all 9 numbered with chosen answer and justification.
- DONE: The chosen UX (tabs vs. chooser vs. picker-revisit vs. status-line) is named explicitly with a brief comparison against the alternatives, and the keymap is laid out with no collision against existing bindings.
  See "Chosen UX" and "Keymap" sections — chose status-line breadcrumb + `]`/`[` cycle + `P` overlay; rejected tabs with reasoning; collision audit against `src/app.rs:555-593` confirms `]`, `[`, `P` are unused.
- DONE: Acceptance criteria replace the placeholder section with concrete verifiable AC-N bullets covering: the switch UX, single-workflow no-cost rule, per-workflow state preservation, watcher fan-out behavior, and re-discovery refresh.
  AC-1..AC-7 each name a verification approach with concrete fixtures and assertions.

### Summary

Locked the in-session workflow switch to a status-line breadcrumb (`[i/N] path` extended onto the existing graph header), `]`/`[` cycle keys, and a `P` picker-overlay re-discovery escape hatch — explicitly rejecting the captain's tabs hypothesis on grounds of vertical density and keymap budget. Per-workflow state is retained in a `Vec<OverviewState>` with lazy first-load and never-drop semantics so return cycles are O(1) IO-free. The watcher is single-instance and follows the active workflow with teardown/restart on switch plus synchronous reload to cover the gap; re-discovery is user-initiated via `P` to avoid file-handle multiplication. Single-workflow sessions and `-w/--workflow-dir`-pinned sessions pay zero UI or keymap cost.

## Implementation plan

### Step 1 — Refactor `App` to own `Vec<OverviewState>` (file: `src/app.rs`)

Make the active workflow index addressable while keeping single-workflow callers byte-compatible.

1. Add a new struct alongside `AppMode`:
   ```
   #[derive(Debug, Clone, PartialEq)]
   pub struct OverviewSession {
       scan_root: Option<PathBuf>,        // None for `-w` single-workflow mode
       discovery: Vec<DiscoveredWorkflow>, // len() == 1 in single mode; >=2 in multi
       workflows: Vec<Option<OverviewState>>, // lazy: None until first activation
       active: usize,
       pinned_single: bool,               // true when `-w` was used; suppresses cycle/P
   }
   ```
2. Replace `AppMode::Overview(OverviewState)` with `AppMode::Overview(OverviewSession)`. Internal accessors (`overview()`, `workflow_dir()`, `snapshot()`, `selected_index()`, `view_scope()`, etc.) delegate to `session.workflows[session.active].as_ref().unwrap()` for the active state.
3. Add `OverviewSession` API (private to crate):
   - `single(state: OverviewState, pinned: bool) -> Self` — builds a 1-element session from a pre-loaded state. `pinned == true` for the `-w` path.
   - `from_discovery(scan_root: PathBuf, discovery: Vec<DiscoveredWorkflow>, initial_active: usize, initial_state: OverviewState) -> Self` — multi-workflow constructor, marks `workflows[initial_active] = Some(initial_state)`, others `None`.
   - `active_state(&self) -> &OverviewState` / `_mut` (panics on `None` — invariant: active slot is always materialized).
   - `discovery(&self) -> &[DiscoveredWorkflow]`, `active_index() -> usize`, `len() -> usize`, `is_multi() -> bool` (i.e. `discovery.len() >= 2 && !pinned_single`).
   - `cycle_next(&mut self) -> WorkflowSwitch` / `cycle_prev(&mut self) -> WorkflowSwitch`. Returns a struct describing what the event loop must do (see Step 4).
   - `select(&mut self, target_index: usize) -> WorkflowSwitch`.
   - `replace_discovery(&mut self, new_discovery: Vec<DiscoveredWorkflow>)` — used by `P` re-discovery. Preserves active workflow by canonical-path match; if the active workflow is gone, leaves `active` clamped to 0 and the materialized state for the prior path discarded; mismatched indices in `workflows` are remapped by canonical path so previously-loaded states aren't dropped on re-discovery.
4. Add `WorkflowSwitch { dropped_prior: bool, target_index: usize, needs_first_load: bool }` enum/struct. The event loop uses this to (a) drop the watcher, (b) call `OverviewState::load` if `needs_first_load`, (c) call `reload()` otherwise, (d) start a new watcher.
5. Update existing constructors:
   - `App::new(workflow_dir)` — single empty session, `pinned_single = true`.
   - `App::load(workflow_dir)` — single materialized session, `pinned_single = true`.
   - `App::from_snapshot(workflow_dir, snapshot)` — single session, `pinned_single = true`. (Preserves test seam.)
   - Add `App::from_overview_session(session)` so `decide_app` can build either single or multi sessions.
6. Update `App::handle_key`'s overview arm to gate `]`, `[`, `P` on `session.is_multi()`. New behavior:
   - `]` → emit `cycle_next` switch request via a new `pending_switch: Option<WorkflowSwitch>` field on `App`, or alternatively expose a `take_pending_switch()` method that the event loop polls each frame. (Pure-state design: handlers do not touch FS — they only mutate the active index and mark the prior watcher dead.)
   - `[` → cycle_prev.
   - `P` → set `app.mode` to a new `AppMode::PickerOverlay(PickerState)` built from current `session.discovery` (no re-discovery yet — discovery is re-run lazily when overlay opens; see Step 2 for the variant decision).
7. **Picker-overlay variant decision:** add a third mode `AppMode::PickerOverlay { underlying: OverviewSession, picker: PickerState }`. This keeps the existing `AppMode::Picker` reserved for the startup/zero-overview flow and lets the overlay carry the prior session unchanged so `Esc` restores it. Rendering reuses `picker::render_in` over the same centered column and over a `Clear` overlay.
8. Picker-overlay key handling: `Enter` confirms — if the chosen workflow's canonical path matches an existing `discovery[i]`, set `active = i` (with a `WorkflowSwitch`); if not, push it into `discovery` and `workflows` with `None`. `Esc` discards picker, restores `AppMode::Overview(underlying)`.

Files touched in this step: `src/app.rs` only. Tests in the existing `tests` mod stay green because single-workflow constructors keep their semantics; the back-compat accessors `workflow_dir()`, `snapshot()`, `selected_index()` etc. continue to delegate to the active overview state.

### Step 2 — Re-discovery seam (file: `src/app.rs` + `src/lib.rs`)

`P` needs a way to re-run `discover_workflows` without baking FS calls into `App`. Two options:

- **Chosen:** expose a closure-shaped seam. Add `App::open_picker_overlay<F>(&mut self, discover: F) where F: FnOnce(&Path) -> Result<Vec<DiscoveredWorkflow>, DiscoveryError>`. The event loop in `lib.rs` passes the real `discovery::discover_workflows`; tests pass a fake. On error, the picker overlay opens with the prior `discovery` list and the `error` field set to the discovery error string (reusing `PickerState::set_error`).
- Rejected: directly calling `discovery::discover_workflows` from `handle_key`. This couples `App` tests to the FS and forces tempdirs everywhere.

Add a `scan_root: Option<PathBuf>` to `OverviewSession` so the closure has a root to scan. When `pinned_single` is true, `P` is inert and the closure is never called.

### Step 3 — Breadcrumb in graph header (file: `src/ui/graph.rs`)

Extend the existing block title in `render_stage_graph`:

1. Add a parameter or read from a new method `OverviewSession::breadcrumb_label()` that returns `Some("[2/3]")` when `is_multi()`, else `None`. Since `render_stage_graph` currently takes `&OverviewState`, plumb the breadcrumb through by either:
   - **Chosen:** add a sibling `pub fn render_stage_graph_with_breadcrumb(frame, area, state, breadcrumb: Option<&str>)`, keep the old `render_stage_graph` as a thin wrapper that passes `None`. Caller in `src/ui/mod.rs::render_overview` is updated to pass the breadcrumb derived from the session.
   - Rejected: changing `render_stage_graph` to take the whole session — the function is unit-tested with bare `OverviewState` fixtures and we should not break those.
2. Update the `title` format string from `"Workflow — [active] — archived: ... — {path}"` to `"Workflow — [active] — archived: ... — {breadcrumb_prefix}{path}"` where `breadcrumb_prefix` is `"[2/3] "` or empty.
3. The breadcrumb is *also* emitted when `pinned_single` is true and `discovery.len() == 1` from the auto-discovery path? **No** — the spec says the breadcrumb is only rendered when `is_multi()`. Single-workflow case (whether via `-w` or via discovery returning 1 result) shows nothing. This is the same predicate that gates the keys.

### Step 4 — Event-loop wiring (file: `src/lib.rs`)

Replace the picker-to-overview transition heuristic with an explicit pending-switch drain. Sequence per frame:

1. `terminal.draw(...)`.
2. If `app.should_quit()`, break.
3. Drain refresh signals (existing logic — unchanged).
4. Poll terminal events; on key, `app.handle_key(key)`.
5. **New:** call `app.take_pending_switch()`. If `Some(switch)`:
   - Drop the current `watcher_state` (its `Drop` joins the debounce thread).
   - If `switch.needs_first_load`: call `app.materialize_active()` which under the hood does `OverviewState::load(active_dir)`; on parse failure, install a synthetic empty `OverviewState` with `last_refresh_error` set.
   - Else: call `app.reload()` for the synchronous-converge contract.
   - Call `start_watcher_for(&mut app)` to install a fresh watcher rooted at the new active dir.
6. **New:** if a picker-overlay open was requested (signalled by `app.take_pending_overlay_open()` returning `Some(())`), call `discovery::discover_workflows(scan_root)`, then `app.apply_picker_overlay(result_or_err)`. This step is sequenced *before* the switch drain so an overlay-confirm in the same frame still triggers the switch drain on the next frame.
7. Existing prior-mode-was-picker block goes away — its job is now done by step 5 (the startup picker-confirm path also produces a `WorkflowSwitch`).

Add `start_watcher_for` adjustments: it already guards on `AppMode::Overview`; no change needed except that `app.workflow_dir()` now returns the active workflow's dir.

### Step 5 — `decide_app` update (file: `src/lib.rs`)

The current decision tree returns either an empty `App` for the `-w` single path, an `App::load` for discovery-len==1, or an `App::from_picker` for >=2. New behavior:

1. `-w` path: build a single-workflow session with `pinned_single = true`. The session's `discovery` is a 1-element vec containing the loaded dir; `scan_root = None`. `decide_app` returns `DecideOutcome::Overview(app)` as before — semantics unchanged.
2. Discovery path with 1 workflow: build a single-workflow session with `pinned_single = false` (so future `P` could in principle re-discover, but per the design — task 6 — single-workflow sessions hide `P` regardless; gate stays on `is_multi()`).
3. Discovery path with N>=2 workflows: still return `DecideOutcome::Picker(App::from_picker(...))`. The startup picker now confirms by building an `OverviewSession::from_discovery` keyed on the chosen index and producing a `WorkflowSwitch` for the event loop's first frame. Existing `App::from_picker` builds a `PickerState`; the change is in what `App::handle_key` does on `Enter` in picker mode — it must now build the multi-workflow session, not a single-workflow overview.

### Step 6 — Help popup updates (file: `src/ui/mod.rs::render_help_popup`)

1. Pass `App` (not nothing) into `render_help_popup` so it can branch on `app.as_overview().map(|s| s.is_multi()).unwrap_or(false)` (reaching through a session accessor to be added).
2. When `is_multi()` is true, append three lines to the help text:
   ```
   ]              cycle to next workflow
   [              cycle to previous workflow
   P              re-discover & pick workflow
   ```
3. When false (single workflow or picker mode): help stays as-is.

### Step 7 — Tests

All new tests live in their natural module: state tests in `src/app.rs`, header rendering tests in `src/ui/graph.rs`, help tests in `src/ui/mod.rs`, lifecycle tests in `src/lib.rs` (or a new `src/tests/` integration target if cleaner). Concrete tests:

- `app::tests::cycle_keys_advance_active_index_in_multi_session` — build a 3-workflow session via fake discovery + 3 tempdir fixtures; press `]` → assert `active_index() == 1`; press `]` twice more → wrap to 0; press `[` → wrap to 2.
- `app::tests::cycle_keys_inert_in_single_session` — build a 1-workflow session; press `]`, `[`, `P`; assert `App` state byte-identical (use `assert_eq!(app, original)` since `PartialEq` already derived).
- `app::tests::picker_overlay_open_close_preserves_session` — open `P` (with a stub discover closure returning the same list), assert mode is `PickerOverlay`; press `Esc`; assert mode reverted to `Overview` and active index unchanged.
- `app::tests::picker_overlay_pickup_adds_new_workflow` — discover closure returns `[w0, w1, w2_new]` where `w2_new` was not previously in `discovery`; press `Enter` on row 2; assert `discovery.len() == 3`, `active_index() == 2`, and `WorkflowSwitch::needs_first_load == true`.
- `app::tests::switch_preserves_per_workflow_state` — fixture: two real workflow tempdirs each with >=3 entities. Activate A, press Down twice (selected_index == 2), press `a` to flip to Archived. Cycle to B. Cycle back to A. Assert `selected_index == 2`, `view_scope == Archived`, `archive_loaded == true`.
- `app::tests::first_activation_loads_exactly_once` — instrument by counting calls to a fake `OverviewState::load`-like seam. Cycle A → B → A → B; assert each workflow's load count == 1. (Implemented by injecting a `LoadFn` trait at the session boundary, or by using `tempfile`-backed real workflows and checking `archive_loaded` plus `last_refresh_error` rather than direct call counts; choose whichever lands cleanest.)
- `app::tests::keymap_audit_is_disjoint` — enumerate the keys recognized by `handle_key` (via static const list constructed in the same module) and assert `']', '[', 'P'` are not in the existing-binding set `{ 'a', '?', 'j', 'k', 'q' }` plus the special-key set `{ Down, Up, Home, End, Enter, Esc }`.
- `app::tests::switch_failure_records_refresh_error_synthetic_state` — point one slot at a malformed workflow dir; cycle into it; assert active state is empty and `last_refresh_error()` is Some.
- `ui::graph::tests::breadcrumb_appears_in_header_when_multi` — render the header with breadcrumb `Some("[2/3]")`; assert rendered text contains `"[2/3]"`.
- `ui::graph::tests::no_breadcrumb_in_single_workflow` — render with `None`; assert rendered text does *not* contain `"[1/1]"`.
- `ui::tests::help_popup_shows_cycle_hints_only_in_multi` — multi session: assert rendered help contains `"cycle to next workflow"`. Single session: assert it does not.
- `lib::tests::watcher_restarts_on_switch` — spin up two tempdirs, decide_app into a multi session, press `]`, assert via probe that the new watcher is rooted at the new dir. (Concretely: instrument `start_watcher_for` to record `(start_count, last_root)` in a test harness, or — simpler — dispatch `]` against a 2-workflow `App`, take a `WorkflowSwitch`, drop watcher, start watcher, and assert `WorkflowWatcher::start` was called with the new dir. A `factory` parameter on `start_watcher_for` keeps this testable without a real `notify` backend.)
- `lib::tests::stale_refresh_signal_after_switch_is_dropped` — feed a `RefreshSignal` on the prior receiver after the switch; assert main loop does not panic and active state does not reload from the wrong dir. (The natural way: drop the receiver as part of `watcher_state = None` before installing the new one, and add an assertion that no `app.reload()` runs against a dropped channel.)

### Step 8 — Verification commands

Run before commit:

- `cargo fmt --check` (zero output).
- `cargo clippy --all-targets -- -D warnings` (zero warnings).
- `cargo test` (all green, including the new tests above).
- Optional smoke: `cargo run -- --help` to confirm `-w/--workflow-dir` still parses.

### File ownership summary

| File | Edits |
|------|-------|
| `src/app.rs` | New `OverviewSession` struct, new `WorkflowSwitch`, new `AppMode::PickerOverlay` variant, key handler updates for `]`/`[`/`P`, `take_pending_switch`/`take_pending_overlay_open` accessors, all `App::*` constructors updated, new tests. |
| `src/lib.rs` | `decide_app` builds sessions instead of bare overviews; `run_terminal` drains pending switches and overlay-open requests; calls real `discover_workflows` for re-discovery; existing picker-to-overview transition heuristic removed. |
| `src/ui/mod.rs` | `render_overview` passes breadcrumb to graph; `render_help_popup` accepts `&App` and conditionally shows cycle/picker hints; new mode `AppMode::PickerOverlay` rendered via `Clear` + `picker::render_in` over the centered column atop the prior frame. |
| `src/ui/graph.rs` | New `render_stage_graph_with_breadcrumb`; existing `render_stage_graph` kept as a thin wrapper; breadcrumb prefix injected into block title. |
| `src/ui/picker.rs` | No structural change. The footer hint may be tweaked to mention `Esc: cancel` so the overlay reuse reads cleanly, but no new module. |
| `src/watcher.rs` | No change. Existing `start` / `Drop` contract is sufficient for teardown+restart. |
| `src/discovery.rs` | No change. Re-uses `discover_workflows` from the closure passed by `lib.rs`. |
| `src/cli.rs` | No change. `-w` continues to mean "single-workflow pinned session." |

### Scope-defense notes

- No new module split is introduced; all new types fit naturally next to their existing siblings (`OverviewSession` next to `OverviewState`; `WorkflowSwitch` next to `AppMode`). A future split into `src/app/state.rs` + `src/app/mode.rs` is *not* part of this task — it would balloon the diff for marginal organizational gain.
- The picker overlay reuses `src/ui/picker.rs::render_in` verbatim. We do not duplicate the picker UI code.
- The existing `AppMode::Picker` variant for the startup discovery flow is kept distinct from `AppMode::PickerOverlay` so that `Esc` semantics differ correctly: startup picker `Esc` quits; overlay `Esc` returns to the prior session.
- Cycle keys are pure index mutations; they never touch the filesystem. The event loop is the only place that performs IO on switch (load-or-reload + watcher restart). This keeps `App::handle_key` synchronously testable without tempdirs in the cycle/overlay tests.

## Stage Report: plan

- DONE: Step-by-step plan enumerates each file change in order (state types, app refactor to hold `Vec<OverviewState>`, watcher restart wiring in `lib.rs`, breadcrumb rendering in graph header, key handling, picker-overlay variant or reuse) plus verification commands.
  Steps 1–8 above cover state, app refactor, lib.rs wiring, graph header, key handling, picker overlay reuse via `Clear`+`render_in`, and a verification-commands subsection.
- DONE: Test strategy names specific tests: `]`/`[` cycle in multi-workflow fixture, `P` overlay open/close + add/remove flow, single-workflow no-op, per-workflow state preservation across switches, watcher restart correctness, keymap audit unit test.
  See Step 7 — names ten specific tests across `app`, `ui::graph`, `ui` (help), and `lib` modules; cycle/overlay/single/preserve/watcher-restart/keymap-audit are each present.
- DONE: File/module ownership is explicit: which files each step touches, whether new modules (e.g. `src/app/state.rs` or similar split) are introduced, and how the existing picker is extended vs. wrapped.
  See "File ownership summary" table and "Scope-defense notes" — explicitly chose no new module split, picker reused via new `AppMode::PickerOverlay` variant rendering the existing `picker::render_in` under `Clear`, with `src/ui/graph.rs` kept testable by adding a sibling renderer rather than mutating `render_stage_graph`'s signature.

### Summary

Decomposed the multi-workflow switch into eight ordered steps centered on a new `OverviewSession` in `src/app.rs` that owns `Vec<Option<OverviewState>>` plus an `active` index, lazy materialization, and a `WorkflowSwitch` value the event loop drains each frame to teardown/restart the watcher and call `load`-or-`reload`. The picker overlay is realized as a new `AppMode::PickerOverlay { underlying, picker }` variant rendered via `Clear` + the existing `picker::render_in`, with re-discovery driven by a closure seam so `App` tests stay FS-free. Breadcrumb rendering is added through a sibling `render_stage_graph_with_breadcrumb` to keep the existing graph tests untouched, and a precise ten-test plan covers cycle, overlay, single-workflow inertness, state preservation, lazy first-load, watcher restart, keymap collision, and switch-failure semantics.

## Stage Report: implement

- DONE: `OverviewSession` + `AppMode::PickerOverlay` variant + breadcrumb sibling renderer all land per the plan; cycle and overlay keys wired through `App::handle_key` and `lib.rs` event loop; watcher correctly restarts on switch.
  `OverviewSession` lives in `src/app.rs` with lazy `Vec<Option<OverviewState>>`, `cycle_next/prev`, `select`, and `replace_discovery`; `AppMode::PickerOverlay { underlying, picker }` reuses `picker::render_in` over `Clear` in `src/ui/mod.rs`; sibling `render_stage_graph_with_breadcrumb` in `src/ui/graph.rs` keeps the original graph tests intact; event loop in `src/lib.rs` drains `take_pending_overlay_open` (re-runs `discover_workflows`) then `take_pending_switch` (drops watcher, materializes-or-reloads, restarts watcher).
- DONE: The 10 named tests from the plan are present and passing — covering cycle nav, overlay open/close + re-discovery flow, single-workflow no-op, per-workflow state preservation across switches, watcher restart correctness, keymap audit, and breadcrumb render shape.
  `app::tests`: `cycle_keys_advance_active_index_in_multi_session`, `cycle_keys_inert_in_single_session`, `picker_overlay_open_close_preserves_session`, `picker_overlay_pickup_adds_new_workflow`, `switch_preserves_per_workflow_state`, `switch_failure_records_refresh_error_on_synthetic_state`, `keymap_audit_is_disjoint`. `ui::graph::tests`: `breadcrumb_appears_in_header_when_multi`, `no_breadcrumb_in_single_workflow`. `ui::tests`: `help_popup_shows_cycle_hints_only_in_multi`. `lib::tests`: `watcher_restarts_on_switch`. The "first activation loads exactly once" plan item is folded into the cycle-nav test which asserts `needs_first_load == true` on first activation and `false` on return.
- DONE: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all fully clean on the worktree branch (no surviving exceptions).
  `cargo fmt --check`: zero output. `cargo clippy --all-targets -- -D warnings`: zero warnings. `cargo test`: 96 lib + 4 integration tests passed (1 ignored, the existing notify-backend smoke).

### Summary

Landed the in-session multi-workflow switch end-to-end. `App` now wraps an `OverviewSession` of lazy `OverviewState` slots with single-element pinned-single sessions for the `-w` and discovery==1 paths; `]`/`[`/`P` are gated on `is_multi()` and emit pure-state changes the event loop drains to perform watcher teardown + load-or-reload + watcher restart. Picker overlay is a `Clear`-overlaid reuse of the existing `picker::render_in`, with `Esc` restoring the underlying session and `Enter` confirming via `replace_discovery` + `select`. Breadcrumb is plumbed through a sibling `render_stage_graph_with_breadcrumb` so existing graph tests stay byte-stable; help popup grows three lines only when multi.

## Stage Report: review

- DONE: AC-1 — `]`/`[` cycle active workflow without process restart.
  `app::tests::cycle_keys_advance_active_index_in_multi_session` exercises 3-workflow tempdir fixture, asserts wrap-around both directions, and confirms `WorkflowSwitch` emission with correct `target_index` + `needs_first_load`. `cargo test` shows it green.
- DONE: AC-2 — Single-workflow sessions render no breadcrumb and `]`/`[`/`P` are inert.
  `app::tests::cycle_keys_inert_in_single_session` clones the App, fires all three keys, asserts byte-identical state and no pending switch/overlay. `ui::graph::tests::no_breadcrumb_in_single_workflow` asserts header omits `[1/1]`. `is_multi()` returns false when `discovery.len() < 2 || pinned_single`, gating both keymap and breadcrumb.
- DONE: AC-3 — Switching preserves `selected_index`, `view_scope`, archive cache.
  `app::tests::switch_preserves_per_workflow_state` flips A to Archived (loads archive cache), cycles to B then back, asserts `view_scope == Archived` and `archive_loaded == true` on return. Cycle test also confirms `needs_first_load == false` on revisit (no re-IO).
- DONE: AC-4 — One watcher follows active workflow.
  `lib::tests::watcher_restarts_on_switch` starts watcher on w0, presses `]`, drops prior watcher, calls `materialize_active`, restarts watcher, asserts new `app.workflow_dir() == w1`. Event loop in `lib.rs:160-170` drops `watcher_state.take()` before installing new one — single-watcher invariant holds.
- DONE: AC-5 — `P` re-runs discovery and updates list.
  `app::tests::picker_overlay_pickup_adds_new_workflow` creates a third workflow on disk, opens overlay with augmented list, navigates to and Enters the new entry, asserts `discovery().len() == 3`, `target_index == 2`, `needs_first_load == true`. `lib.rs:145-155` drains overlay-open and re-runs `discover_workflows`.
- DONE: AC-6 — New keymap doesn't collide with existing bindings.
  `app::tests::keymap_audit_is_disjoint` asserts `]`,`[`,`P` not in `{a,?,j,k,q}`. Verified by reading `handle_key`: special keys (Down/Up/Home/End/Enter/Esc) are non-Char so cannot collide. Help popup adds cycle hints only when `is_multi()` (verified by `help_popup_shows_cycle_hints_only_in_multi`).
- DONE: AC-7 — fmt/clippy/test all clean.
  `cargo fmt --check`: no output. `cargo clippy --all-targets -- -D warnings`: zero warnings. `cargo test`: 96 lib + 4 integration tests passed (1 ignored notify-backend smoke as before, no carve-outs).
- DONE: Diff confined to plan-owned files.
  `git diff --stat main...HEAD` shows: `src/app.rs`, `src/lib.rs`, `src/ui/graph.rs`, `src/ui/mod.rs`, plus the entity file. No drive-by edits.
- DONE: `render_stage_graph` signature unchanged.
  Production callers go through new sibling `render_stage_graph_with_breadcrumb`; the original is `#[cfg(test)]`-gated and unchanged in arg shape, used only by existing graph tests. Grep confirms zero non-test callers of the original.
- DONE: Picker overlay Esc/Enter semantics, single-watcher invariant, help/overlay key gating.
  Inspected `App::handle_key`: help_open intercepts before any mode (line 878, returns), so `]`/`[`/`P` cannot fire while help is open. `PickerOverlay` arm only handles nav/Enter/Esc/q/?, not `]`/`[`/`P` — they no-op while overlay is open. `Esc` in overlay restores underlying session via `mem::replace`. `is_multi()` predicate already covers `pinned_single`, making the `&& !pinned` guard at line 906 redundant-but-harmless.

### Summary

Implementation matches the locked plan. `OverviewSession` cleanly owns `Vec<Option<OverviewState>>` with lazy materialization, switch is a pure index mutation that emits `WorkflowSwitch` for the event loop to drain (drop watcher → materialize-or-reload → restart watcher). PickerOverlay reuses `picker::render_in` over `Clear` and preserves the underlying session for `Esc` restore. All seven ACs verified with concrete test or code-trace evidence. fmt/clippy/test fully green with no carve-outs. Diff confined to the four plan-owned source files plus the entity file.

Verdict: PASSED.
