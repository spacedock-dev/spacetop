---
id: 005
title: Auto-discover Spacedock workflows under `docs/`
status: design
source: captain feedback after build-initial-tui-overview
started: 2026-04-24T16:06:48Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

When `spacetop` is launched without an explicit `--workflow-dir` / `-w`, it should discover Spacedock workflow directories in the current repository and present them to the user. A single repository can host multiple workflows (for example `docs/spacetop-dev/` today, plus future product or research workflows), and the TUI must let the user pick which to open when more than one is found. When exactly one workflow is found, it opens automatically so routine single-workflow repos feel like "just run `spacetop`".

## Problem statement

Today `spacetop` defaults `--workflow-dir` to `.`, which only works when the user runs it from inside a workflow directory. Users at the repo root see a parser error (no `README.md` frontmatter with `stages:`) instead of their workflows. This is the first UX friction a new SpaceTop user hits.

The fix is to match the behavior already proven by `spacedock`'s `status --discover` helper: walk the repo tree, find directories whose `README.md` frontmatter carries `commissioned-by: spacedock@…`, and treat those as the candidate workflow set. We do **not** reinvent the discovery rule — we call through to / mirror the same rule so one repo's "these are my workflows" answer is consistent across tooling.

## Discovery mechanism (locked)

- **Signal:** a directory is a Spacedock workflow when it contains a `README.md` whose YAML frontmatter has a `commissioned-by:` value starting with the literal string `spacedock@` (matches `spacedock@0.10.1`, `spacedock@1.0`, and bare `spacedock@`). No other signal (sentinel file, `_mods/` presence, path name) is used.
- **Scan root:** the git repository root, resolved by walking up from the CWD until a `.git` directory or file is found; if no git root is found, the CWD is used. This matches `status --discover`'s default `--root` behavior.
- **Scope / recursion:** the scan is a full recursive walk from the scan root, **not** limited to `docs/`. `docs/` is the conventional home (and the task title reflects that convention) but we do not hard-code it — a workflow at `pipelines/releases/` or at the repo root must also be found. The walk prunes these directories by name at any depth: `.git`, `.worktrees`, `node_modules`, `vendor`, `dist`, `build`, `__pycache__`, `tests`. Symlinks are followed with cycle protection (track visited real paths). Results are deduplicated on `realpath` and sorted.
- **Archive / mods safety:** because discovery matches on `README.md` + `commissioned-by:` frontmatter, `_archive/` and `_mods/` inside a workflow do not themselves match (they hold entity files, not scaffolding READMEs). No special-case needed.

## User flow (locked)

Given the set of discovered workflows `W`:

1. **`|W| == 0` (zero workflows found):** do not panic, do not silently open an empty TUI. Print a single-line human-readable error to stderr naming the scan root and exit with a non-zero status. Exact message (stable, testable): `spacetop: no Spacedock workflows found under <scan-root>. Pass --workflow-dir <path> to open a specific directory.` No TUI is drawn. No alternate-screen enter/leave.
2. **`|W| == 1` (single workflow found):** auto-open that workflow. Behavior from this point is identical to running `spacetop --workflow-dir <that-path>`. No picker, no extra keypress.
3. **`|W| >= 2` (multiple workflows found):** show a picker screen as the first thing the user sees, before the overview. Selecting a workflow transitions the TUI to the existing overview rendered against that workflow.

**Explicit `-w` / `--workflow-dir` bypass (locked):** when the user passes `-w <path>` or `--workflow-dir <path>` on the CLI, discovery is **not** run at all. The named path is loaded directly, exactly as today. An explicit empty string is still forwarded to the loader and errors there; this stage does not add new validation on the explicit path. (Task 004 adds the `-w` short flag; this task assumes it exists and treats `-w` as an alias for `--workflow-dir` everywhere in this document.)

## Picker UX (locked)

The picker is a **separate screen** (not a side panel, not a startup prompt). It replaces the overview's `Frame` for its entire lifetime; once a workflow is picked, the overview replaces the picker. This keeps picker rendering and overview rendering independent and makes the `App` state machine a simple two-variant enum.

Picker rendering:

- Title bar: `spacetop — pick a workflow` plus the scan root in muted style.
- A single vertical list of workflow entries, one per row, using the same highlight style as the overview's entity list. Each row shows, left to right:
  - the workflow root displayed relative to the scan root (e.g. `docs/spacetop-dev`), falling back to the absolute path if `realpath` is outside the scan root
  - a dim secondary cell with the workflow title (README H1 heading) when cheap to read; if reading the title fails, the cell is blank rather than erroring
- Footer hint: `↑/↓ or j/k: move · Enter: open · q/Esc: quit`.

Picker keybindings:

- `↑` / `k`: move selection up (clamped at 0).
- `↓` / `j`: move selection down (clamped at `len-1`).
- `Home`: jump to first row.
- `End`: jump to last row.
- `Enter`: load the selected workflow and switch to the overview. If loading fails (e.g. malformed README), stay on the picker and render the error in a status line at the bottom; do not quit.
- `q` / `Esc`: quit (sets `should_quit`).

Selection state: initial `selected_index = 0`. There is no "remembered" selection — each process launch starts at index 0. There is no back-out path from overview to picker in this task; pressing `q`/`Esc` in the overview quits as today. (Returning to the picker is a deliberate non-goal for this task to keep the surface small; it can be added later without breaking these ACs.)

## Acceptance criteria

**AC-1 — Running `spacetop` without `-w` from a repo containing multiple workflow directories opens a picker listing each discovered workflow.**
Verified by: integration test against a fixture repo whose git-root contains at least two directories with `README.md` frontmatter starting with `commissioned-by: spacedock@`. The test asserts that the initial frame is the picker (not the overview) and that each fixture workflow path appears as a picker row.

**AC-2 — Running `spacetop` without `-w` from a repo containing exactly one workflow directory auto-opens that workflow's overview (no picker frame is drawn).**
Verified by: integration test against a single-workflow fixture; assert the first frame is the overview for that workflow and that the app's workflow_dir equals the discovered path.

**AC-3 — Running `spacetop` without `-w` from a repo containing zero workflow directories exits non-zero with the stable stderr message and draws no TUI.**
Verified by: integration test against a fixture repo with no `commissioned-by:` READMEs; assert non-zero exit status, assert stderr contains the literal prefix `spacetop: no Spacedock workflows found under ` and the scan-root path, and assert no alternate-screen escape sequences are emitted on stdout.

**AC-4 — Explicit `-w <path>` and `--workflow-dir <path>` both bypass discovery and load the named directory directly.**
Verified by: CLI-level test that runs `spacetop -w <single-workflow-path>` and `spacetop --workflow-dir <single-workflow-path>` against a fixture that *also* contains a second discoverable workflow; assert the loaded workflow_dir is the one passed on the CLI and that discovery was not invoked (no picker frame, no walk of the second workflow).

**AC-5 — In the picker, pressing Enter on a selected workflow transitions to that workflow's overview; pressing `q` or `Esc` quits without transitioning.**
Verified by: unit tests on the picker app state (drive `handle_key` with `Enter`, `q`, `Esc`, `↓`, `↑`, `Home`, `End`) plus one render-snapshot test confirming the picker frame shape and the post-Enter overview frame.

**AC-6 — Discovery honors the pruned directory list (`.git`, `.worktrees`, `node_modules`, `vendor`, `dist`, `build`, `__pycache__`, `tests`) and deduplicates by realpath.**
Verified by: unit test on the discovery function against a fixture tree that includes a workflow under a pruned directory (not returned) and a symlinked duplicate of a workflow (returned once, as its canonical realpath).

## Parser/TUI constraints (locked)

### Where discovery lives

- A new module `src/discovery.rs` owns the walk-and-match logic. It exposes a pure function `discover_workflows(root: &Path) -> Result<Vec<DiscoveredWorkflow>, DiscoveryError>` whose signature is independent of clap and independent of the `App`. `DiscoveredWorkflow { root: PathBuf, title: Option<String> }` is the new domain type. `title` is best-effort (read the `README.md` H1 if cheap; `None` on any IO or parse miss — title failure is never a discovery error).
- Discovery mirrors the pruning list and matching rule from `status --discover` verbatim. The prune set is a single `const` in the new module so future divergence is grep-able.
- `lib.rs::run` is the only place that decides between "explicit `-w`" and "discovery" paths. The decision flow is:
  1. If `cli.workflow_dir` was supplied explicitly (clap distinguishes default-value vs. user-supplied via `ArgMatches::value_source`), call `App::load(path)` and go straight to `run_terminal`.
  2. Otherwise resolve the scan root (git-root walk-up, fall back to CWD), call `discover_workflows`, then branch on `len()`:
     - `0`: print the stderr message and return a non-zero error.
     - `1`: `App::load(discovered[0].root)` and `run_terminal`.
     - `>=2`: construct a picker-mode `App` and `run_terminal`.
- CLI parsing does not get a new flag in this task. The default-value behavior of `--workflow-dir` changes from "the CWD literal `.`" to "unset, triggers discovery." The cleanest way is to switch `workflow_dir` to `Option<PathBuf>` (no `default_value`); callers that want today's "." behavior pass `-w .` explicitly. This is a small, contained change to `src/cli.rs` but it is a breaking behavior change for `spacetop .` vs. bare `spacetop`, which is the intended behavior difference.

### How it feeds into `App`

- `App` grows from a single overview state into a two-state machine. Suggested shape (names to be refined in `plan`):
  - `enum AppMode { Picker(PickerState), Overview(OverviewState) }`
  - `PickerState { workflows: Vec<DiscoveredWorkflow>, selected_index: usize, error: Option<String> }`
  - `OverviewState` holds today's `snapshot`, `workflow_dir`, `selected_index`.
  - `App::handle_key` dispatches on `mode`. The overview branch's behavior is unchanged from today.
- `App::load(path)` stays as the single-workflow entry point. A new constructor `App::from_picker(Vec<DiscoveredWorkflow>)` builds the picker-mode variant. Transitions from picker to overview call `OverviewState::load(path)` internally; load errors surface into `PickerState.error` rather than panicking.
- Existing overview tests and behavior must continue to pass without modification — the picker is strictly additive. The overview's `stage_counts`, `selected_item`, `selected_index`, and navigation accessors keep their current public shape (tests may need to reach through `App::as_overview()` or similar, but the overview semantics do not change).

### Rendering

- `ui::render` grows a top-level match on `app.mode()` and delegates to `ui::picker::render` or the existing overview render. The picker renderer is new code in a new submodule; the overview renderer is untouched.
- The existing `Terminal` / alternate-screen lifecycle in `run_terminal` is reused for both modes. The zero-workflow path does **not** call `run_terminal` — it returns an error before `enable_raw_mode`, so no alternate-screen flicker on failure.

### Tests

- Unit tests live next to their modules (`src/discovery.rs`, `src/app.rs`, `src/ui/picker.rs`).
- Integration tests that need a fixture repo tree use `tempfile::tempdir()` and write the minimum files (empty `.git` dir, a couple of `README.md`s with the right frontmatter) rather than checking fixtures into the repo.
- No dependency on invoking the `status --discover` subprocess at runtime — SpaceTop owns its walker in Rust. The Python script is the reference, not a runtime dependency.

## Coordination with task 004

Task 004 (`add-workflow-dir-short-flag`) lands the `-w` short alias for `--workflow-dir`. This design assumes `-w` exists. If 004 has not merged by the time this task's `implement` stage begins, the implementer should either rebase on 004 or add the short flag themselves (it is a one-line clap change) — either way is acceptable. This task's ACs refer to both `-w` and `--workflow-dir`.

## Stage Report: design

- DONE: Problem statement, discovery mechanism (what signal identifies a workflow dir, scan scope, recursion policy), and user flow (zero/single/multiple workflows found, picker UX) are locked.
  Problem statement, "Discovery mechanism (locked)", "User flow (locked)", and "Picker UX (locked)" sections name the signal (`commissioned-by: spacedock@…`), scan root (git root walk-up with CWD fallback), recursion (full tree walk with pruned dir list), and UX for all three |W| cases.
- DONE: Acceptance criteria replace the placeholder section with concrete, verifiable AC-N bullets covering multi-workflow discovery, single-workflow auto-open, zero-workflow fallback, explicit `-w/--workflow-dir` bypass, and picker behavior.
  AC-1 through AC-6 replace the placeholder block; each names a verification strategy (integration test, CLI test, unit test, snapshot test) and a fixture shape.
- DONE: Parser/TUI constraints are named — where discovery lives (CLI layer vs. app layer), how it feeds into `App::load`, and any new domain types the parser exposes.
  "Parser/TUI constraints (locked)" names `src/discovery.rs` as the new module, `DiscoveredWorkflow` as the new domain type, `lib.rs::run` as the CLI-vs-discovery branch point, and the `AppMode::{Picker, Overview}` two-state machine with `App::from_picker` as the picker entry point.

### Summary

Locked the discovery signal on `commissioned-by: spacedock@…` in a workflow's README frontmatter (matching `status --discover` exactly so the two tools never disagree), scoped the scan to the git-root walk with the same pruned directory list, and picked a full-screen picker for the multi-workflow case with auto-open for single and non-zero exit + stable stderr message for zero. Explicit `-w/--workflow-dir` bypasses discovery entirely. The implementation will add a new `src/discovery.rs` module and convert `App` into a two-state `Picker` / `Overview` machine; no changes to the existing parser or overview renderer are required.
