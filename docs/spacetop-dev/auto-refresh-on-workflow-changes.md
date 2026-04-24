---
id: 008
title: Auto-refresh task list and detail when workflow files change
status: design
source: captain feedback during 006 planning
started: 2026-04-24T16:13:15Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

## Problem statement

The SpaceTop TUI currently loads `WorkflowSnapshot` exactly once, inside
`App::load` (see `src/app.rs::App::load` calling `parser::load_workflow_dir`),
invoked from `lib.rs::run` before the terminal loop starts. Once the event
loop in `run_terminal` is running it only reacts to crossterm key events; no
code re-reads the filesystem. So anything that changes workflow files while
SpaceTop is open — the captain editing a task in their editor, a dispatched
worker committing a stage report, a concurrent first-officer session
promoting status — is invisible until the user quits and restarts. For a
workflow inspection TUI this is a correctness problem, not a polish problem:
the panel actively lies about current state.

The feature: watch the workflow directory for file change events while the
TUI is running, re-parse the snapshot when changes settle, and swap the
in-memory `WorkflowSnapshot` on the main thread so the task list, stage
counts, and detail pane all update live without losing the user's current
selection.

## Target user flow

1. User runs `spacetop docs/spacetop-dev` and the TUI opens on the first
   task.
2. In another terminal or editor, someone edits `add-archived-tasks-view.md`
   (changes status, edits body, rewrites frontmatter).
3. Within roughly 300-500 ms of the editor's final write, the TUI's task
   list, stage-ribbon counts, and — if that task is selected — the detail
   pane reflect the new content. No user keypress required.
4. The same flow works for: creating a new task file, deleting a task file,
   renaming a task file (git mv), and (if archived scope is enabled, see
   below) moving a file into or out of `_archive/`.
5. If the user has a task selected and that task still exists after the
   refresh, their selection stays on the same task by slug. If the task is
   gone, selection falls back to a sensible neighbor (see selection policy
   below).
6. If a refresh fails (half-written frontmatter, YAML parse error, one file
   temporarily missing mid-rename), the TUI keeps rendering the last good
   snapshot and surfaces a non-fatal indicator in the status bar rather than
   crashing or blanking out.

## Design decisions (open questions resolved)

### Watcher crate: `notify` v8, recommended backend

Recommendation: add `notify = "8"` as a dependency in the `implement` stage
(do NOT add it in this design stage). `notify` is the canonical Rust
cross-platform file-watching crate; its default backend selects
FSEvents on macOS, inotify on Linux, and ReadDirectoryChangesW on Windows.
No non-default features are required for our use case (recursive directory
watch of a small workflow dir). We do NOT plan to use `notify-debouncer-full`
or `notify-debouncer-mini` — we debounce in our own code so we can keep a
single, testable debounce model and avoid a second dependency surface. If
the platform's native watcher fails to initialize (e.g., inotify descriptor
exhaustion), `notify` can be configured to fall back to its `PollWatcher`
at a 1 s interval; we enable that fallback when `RecommendedWatcher::new`
returns `Err`, and log a one-line notice into the status bar.

### Event scope: recursive watch of the workflow directory

Watch the workflow root recursively (`RecursiveMode::Recursive`). Concretely
that covers, for today's layout:

- top-level `*.md` task files
- `_archive/*.md` (for task 006 compatibility)
- `_mods/` modification blocks
- any future folder-form `{slug}/index.md` layout (we pre-emptively cover
  this so task 007 / future expansions don't need to re-plumb the watcher)

We filter events in our own code rather than subscribing to targeted globs,
because `notify` does not take glob filters and because recursive watch on
a workflow dir of tens-to-hundreds of files is cheap. Event filtering rule:
only trigger a reload when the changed path either (a) ends in `.md`, or
(b) is a directory create/remove whose name matches `[A-Za-z0-9._-]+` (so
we react to `_archive/` appearing or a folder-form task being added). Other
file types (swap files from `vim`, `.DS_Store`, editor backups) are
ignored.

### Debounce window: 250 ms coalescing

Editors emit event bursts: `vim` does write-temp + rename + chmod, VS Code
does write + sometimes a second fsync write, `git` operations touch many
files in rapid succession. Use a 250 ms trailing-edge debounce: on the
first qualifying event, start a 250 ms timer; any additional qualifying
event resets the timer; when the timer expires, trigger one reload. 250 ms
is short enough to feel live to a human editor and long enough to coalesce
an atomic-save burst and a multi-file `git checkout`. The debounce window
is a single config constant and will be exposed to tests via a builder
argument so tests can set it to 0 ms and drive the refresh deterministically.

### Plumbing model: background thread with an mpsc channel, polled by the main loop

Architecture:

- A `WorkflowWatcher` struct owns the `notify::RecommendedWatcher` and a
  thread. The thread receives raw `notify::Event`s, applies the path filter,
  applies the 250 ms debounce, and on each debounce-fire sends a single
  `RefreshSignal` message through an `mpsc::Sender<RefreshSignal>`.
- The main loop in `lib.rs::run_terminal` owns the matching `Receiver`.
- Replace the current `event::poll(Duration::from_millis(250))` with a tick
  loop that: (a) drains any pending `RefreshSignal`s from the channel using
  `try_recv`, calls `App::reload()` for each, then (b) polls crossterm for
  key events with a short timeout (e.g., 100 ms). This keeps the existing
  crossterm reader intact and avoids any attempt to merge the watcher
  channel into crossterm's own event source (which would require either a
  second thread funnelling into a unified channel or a platform-specific
  mux — both overkill for this task). The 100 ms crossterm poll keyframes
  keystroke responsiveness; the `try_recv` keeps refreshes prompt.
- `App::reload()` is a new method that re-runs `parser::load_workflow_dir`
  against `self.workflow_dir`, and on success merges the new snapshot into
  `self` preserving selection (see below). On parse failure it leaves the
  existing snapshot in place and records a `last_refresh_error: Option<String>`
  for the UI to surface.
- For deterministic tests, `WorkflowWatcher` exposes a constructor that
  accepts a pre-built `(Sender, Receiver)` pair so tests can send synthetic
  `RefreshSignal`s without touching the filesystem, and a separate one that
  wires up the real `notify` watcher. Tests drive `App::reload()` directly
  plus test `App` state after manually injecting a signal through the
  receiver path — no real FS events required.

### Selection preservation policy: keep-by-slug, fall back to nearest index

On every successful reload:

1. Capture the slug (path stem) of `App::selected_item()` before swapping.
2. After swapping in the new snapshot, search the new `items` for a matching
   slug. If found, set `selected_index` to that position.
3. If not found, clamp the prior `selected_index` to the new `items.len()`
   (saturating to 0 for empty workflows). This keeps the cursor in roughly
   the same place when a task is deleted, rather than jumping to the top.
4. Scope toggle interaction (task 006): if task 006 adds `ViewScope::Active`
   / `ViewScope::Archived`, selection preservation runs against the
   currently-visible scope's item list. When the watched event is a file
   moving between active and archive directories, slug-matching naturally
   loses the selection (the task is no longer in the current scope) and
   falls back to the clamped index — which is the desired behavior.

### Error handling

- **Parse error in a single file** (half-written frontmatter during save):
  `parser::load_workflow_dir` already returns `Result<_, ParseError>`. On
  `Err`, `App::reload()` discards the error's snapshot result, keeps the
  prior good snapshot, stores a short error string
  (`last_refresh_error: Option<String>`) on `App`, and schedules no retry —
  the next real FS event will retry naturally. The UI renders the error as
  a one-line status hint; it is cleared on the next successful reload.
  Future refinement (out of scope here): make the parser return partial
  results so one bad file doesn't hide all the good ones, but that is a
  parser change, not a refresh change.
- **File deleted mid-render**: the render function already works off
  `&App::snapshot()`, which is a consistent in-memory copy; a file being
  deleted on disk cannot corrupt the current frame. The delete will surface
  as a watcher event and trigger a reload on the next tick.
- **Watcher backend failure at startup**: fall back to `notify::PollWatcher`
  at 1 s polling interval and surface a one-line status hint
  `watcher: polling fallback`.
- **Watcher thread panics at runtime**: the main loop detects a
  disconnected channel (`try_recv` returning `Disconnected`) and continues
  without live refresh, surfacing `watcher: disconnected` in the status
  bar. The TUI remains usable; the user can quit and restart to recover.

### Archived entities: in scope

Archived entities (`_archive/*.md`) are in scope for this watcher. The
watcher watches recursively so archived files are seen for free. Whether
they are rendered is governed by task 006's `ViewScope` — refresh merely
keeps whatever list the user is currently viewing accurate. Concretely this
means: if the user toggles to archived view, a captain archiving a task
from another terminal will see the archived task appear within 250-500 ms.

## Parser / TUI constraints

- **Where the watcher lives**: a new module `src/watcher.rs` exposing
  `WorkflowWatcher` (owns `notify::RecommendedWatcher` + thread + sender)
  and the `RefreshSignal` type. `lib.rs::run_terminal` constructs one,
  holds the receiver, and drops the watcher on terminal exit (Drop
  implementation on `WorkflowWatcher` joins/stops the thread cleanly).
- **How events enter the main loop without blocking the crossterm reader**:
  non-blocking `Receiver::try_recv` between crossterm `event::poll`
  cycles. Specifically: per tick, drain all queued `RefreshSignal`s with
  `try_recv` until `Empty`, then call `event::poll(100ms)`. This keeps the
  crossterm reader on its own thread (the main thread) and never blocks
  waiting on the watcher channel, which is a common deadlock shape when
  naively merging two event sources.
- **Cost for task 007 (workflow graph view)**: each refresh is a full
  re-parse of the workflow dir plus one swap of `App.snapshot`. For small
  workflows (<200 files) this is sub-millisecond disk work plus YAML parse
  cost. The stage ribbon in task 007 redraws from the new snapshot on the
  next frame with no additional work — it is already a pure function of
  `WorkflowSnapshot`. The 250 ms debounce guarantees at most ~4 reloads
  per second under continuous editing, which is well below any rendering
  or parsing budget concern.
- **Cost for task 006 (archived view)**: `ViewScope` is a render-time
  filter over `snapshot.items`; refresh doesn't need to know which scope
  is active. Selection preservation runs against the visible scope as
  described.
- **No new dependencies in this stage**: `notify` will be added in the
  `implement` stage. Confirmed via `Cargo.toml` — today we have
  `anyhow, clap, crossterm, ratatui, serde, serde_yaml, thiserror`.

## Acceptance criteria

**AC-1 — External writes trigger a reload within a bounded time window.**
Given the TUI is running against a fixture workflow directory, when a test
writes a change to an existing `*.md` task file through the filesystem,
then within 750 ms (250 ms debounce + 500 ms slack for CI jitter) the
TUI's in-memory `WorkflowSnapshot` reflects the new content. Verified by
an integration test that creates a temp workflow dir, starts the watcher,
modifies a file, and asserts on the `App` state after draining the refresh
channel.

**AC-2 — Selection is preserved across refresh when the task still exists.**
Given the user has selected a task with slug `S`, when a refresh is
triggered and `S` is still present in the new snapshot (possibly at a
different index), then after the refresh `App::selected_item()` still
returns the task with slug `S`. Verified by an `app.rs` unit test driving
`App::reload_from_snapshot(new_snapshot)` with a synthetic snapshot that
reorders items; no real filesystem required.

**AC-3 — Parse errors on a single file do not crash the TUI; the prior
snapshot is retained.**
Given a workflow is loaded and the TUI is running, when a file is saved
with invalid YAML frontmatter and a refresh fires, then `App::snapshot()`
still returns the prior good snapshot and `App::last_refresh_error()`
returns `Some(_)` with a short human-readable message. Verified by a unit
test that calls `App::reload()` against a temp workflow dir whose state has
been poisoned, and asserts on both snapshot identity and error state.

**AC-4 — Deterministic-testable refresh path.**
The refresh pipeline exposes a seam for tests: `App::reload_from_snapshot`
(or equivalent) accepts a pre-built `WorkflowSnapshot` and applies the
selection-preservation policy without touching the filesystem. Tests MUST
be able to cover AC-2 without relying on real FS events. Verified by the
AC-2 test itself consuming this API.

**AC-5 — Watcher backend failure degrades gracefully.**
If `notify::RecommendedWatcher::new` returns `Err` on startup, the TUI
either (a) starts with a `PollWatcher` fallback, or (b) starts with no
live refresh and surfaces a status hint — in either case the TUI remains
usable and does not exit. Verified by a unit test that forces the
watcher-construction path to fail (e.g., watching a non-existent dir) and
asserts that `run_terminal`-equivalent setup still yields a functioning
`App`.

**AC-6 — Scope-consistent with task 006.**
When task 006's `ViewScope` ships, refreshes do not reset the scope
toggle, and selection preservation runs against the currently-visible
scope's items. Verified in task 006's feedback pass once both are merged;
this task's implementation MUST NOT hard-code a single scope into the
reload path.

## Stage Report: design

- DONE: Problem statement and user flow are locked: watcher crate recommendation, event scope/globs, debounce window, plumbing model (thread + channel), and behavior on parse failure.
  `notify` v8 recommended (no feature flags); recursive watch of workflow root with `.md`-and-directory path filter; 250 ms trailing-edge debounce; background thread plus `mpsc::Receiver` drained via `try_recv` in the main loop; parse errors retain prior snapshot and surface `last_refresh_error` in the status bar.
- DONE: Acceptance criteria replace the placeholder section with concrete, verifiable AC-N bullets covering: external write triggers reload in bounded time, selection preserved across refresh, parse error tolerance, and a deterministic-testable refresh path (e.g., inject an event for tests rather than relying on real FS events).
  AC-1..AC-6 rewritten with concrete given/when/then shapes and explicit test seams (`App::reload_from_snapshot`, `App::last_refresh_error`, forced-fail watcher construction for AC-5).
- DONE: Parser/TUI constraints are named — where the watcher lives, how events enter the event loop without deadlocking the crossterm reader, and interaction with tasks 006/007.
  Watcher placed in new `src/watcher.rs`; events reach the main loop via non-blocking `try_recv` between `crossterm::event::poll(100ms)` ticks; task 006 `ViewScope` handled as a render-time filter and task 007 stage ribbon redraws from the same snapshot with no extra work.

### Summary

Resolved all open design questions: `notify` v8 recursive watch, 250 ms
debounce, background-thread-plus-mpsc plumbing, keep-by-slug selection with
clamp fallback, snapshot-retention on parse error, and archived entities
in scope to stay consistent with task 006. Acceptance criteria upgraded
from a placeholder to six verifiable AC-N bullets, including an explicit
deterministic test seam (`App::reload_from_snapshot`) so AC-2 doesn't
depend on real filesystem events. No code, no dependencies added — the
`notify` dep will be introduced in `implement`.
