---
id: 038
title: Show worktree-only tasks and body diffs in the task list
status: review
source: captain request 2026-05-12
score:
worktree: .worktrees/spacedock-ensign-038-show-worktree-tasks-with-diff
issue:
pr:
started: 2026-05-12T07:30:23Z
---

The SpaceTop overview currently lists tasks discovered only from the workflow's root directory. When a task is being worked on in an isolated worktree (e.g., `.worktrees/<slug>/docs/spacetop-dev/<slug>.md` or `.claude/worktrees/<name>/docs/spacetop-dev/<slug>.md`), edits to that task's body and frontmatter live on the worktree branch and are invisible to a captain browsing the main checkout. Spacetop should also scan worktree copies of the workflow so worktree-only tasks appear in the list, and so the preview surfaces where the in-flight body diverges from `main`.

Behavior:

- The task list includes tasks that exist only in a worktree copy of the workflow directory. These rows are visually marked as the "worktree version" so the captain can tell them apart from main-tracked tasks.
- When the same task (matched by slug or id) exists in both the root workflow directory and one or more worktree copies, the row uses the **root** copy's frontmatter (status, title, score, etc.) — the main branch remains the source of truth for state display.
- When the root and worktree copies of a task have different bodies, the preview pane shows a diff between the two bodies so the captain can see what the in-flight work has changed.
- Spacetop remains read-only: scanning worktrees never mutates workflow files in either location.

## Acceptance criteria

**AC-1 -- Worktree-only tasks appear in the task list with a distinguishing marker.**
Verified by: an integration test that builds a fixture with a task file present in a worktree copy of the workflow directory but not in the root workflow dir, runs discovery + overview state construction, and asserts the rendered task list contains the task with a visible marker (badge, suffix, or column indicator) that distinguishes worktree-sourced rows from root-sourced rows. The exact label/style is chosen during the design stage.

**AC-2 -- When a task exists in both root and worktree, status uses the root copy.**
Verified by: a unit test in the discovery/parser layer that supplies the same slug from both a root path and a worktree path with different `status:` values in their frontmatter, and asserts the merged task surfaced to the app uses the root copy's `status` (and other frontmatter fields) for list display, while still recording that a worktree copy exists.

**AC-3 -- Preview shows a body diff when root and worktree copies differ.**
Verified by: a test in `src/app.rs` / `src/ui/` (or wherever preview state lives) that constructs a task with a root body and a divergent worktree body, opens the preview, and asserts the rendered preview content contains a diff representation (added/removed lines or equivalent unified-diff structure) of the two bodies rather than only the root body. When bodies are identical, the preview falls back to the existing single-body rendering.

**AC-4 -- Worktree scanning is read-only and does not mutate workflow files.**
Verified by: the existing read-only invariant tests (or a new assertion) confirm that discovering and merging worktree task copies performs no writes under either the root workflow directory or the scanned worktree paths.

**AC-5 -- Worktree discovery handles the absence of worktree directories and missing per-worktree workflow paths gracefully.**
Verified by: a discovery-layer test that exercises (a) a repository with no worktrees registered, (b) a worktree that does not contain the workflow directory at all, and (c) a worktree containing an unrelated subset of task files; asserts no errors are raised and the task list matches the root view in each case (no spurious rows, no panics, no IO errors surfaced to the user).

## Plan

### Worktree enumeration approach

Continue scanning the filesystem (no `git worktree list` shell-out). The current
implementation in `src/parser/worktree.rs::scan_worktrees` only walks
`<repo_root>/.worktrees/*`. Extend it to also walk
`<repo_root>/.claude/worktrees/*`. Both conventions are documented in CLAUDE.md
context and either may be in use on a given checkout. Rationale: avoids adding a
runtime git dependency, keeps tests hermetic (no `git init` required in
fixtures), and matches the existing prune list in `src/discovery.rs` which
already excludes `.worktrees` from workflow discovery. We will also extend that
prune list to include `.claude` so a worktree's mirrored workflow does not
appear as a second top-level workflow in the picker.

The walk remains strictly read-only — `scan_worktrees` only calls `fs::read_dir`
and `fs::read_to_string` (via `parse_work_item`). No `OpenOptions::write`,
`fs::write`, `fs::create_dir`, or `fs::remove*` calls are added on any code
path. AC-4 is preserved by construction; a test asserts the worktree paths'
file mtimes are unchanged after a full `load_workflow_dir` round-trip.

### Testable units & file ownership

Three units, each independently testable without a terminal:

1. **Worktree enumeration & parsing (parser layer)** — `src/parser/worktree.rs`.
   - Add a `worktree_roots(repo_root) -> Vec<PathBuf>` helper that returns the
     union of `.worktrees/*` and `.claude/worktrees/*` directory entries (each
     unique by canonical path, missing parents OK).
   - Change `scan_worktrees` to iterate `worktree_roots(repo_root)` instead of
     hardcoded `.worktrees`. Each returned `WorkItem` carries the worktree path
     in `item.path` exactly as today.

2. **WorkItem source tagging & body-diff data (domain + merge)** —
   `src/domain/mod.rs` + `src/parser/worktree.rs`.
   - Extend `WorkItem` with two non-breaking optional fields:
     - `worktree_source: Option<PathBuf>` — set to the worktree file path when
       this row is sourced from (or has a divergent copy in) a worktree.
     - `main_body: Option<String>` — original root body when a divergent
       worktree body replaced it during merge; `None` when bodies match or
       there is no root copy.
   - Update `merge_worktree_items`:
     - Worktree-only items keep their own body and set
       `worktree_source = Some(item.path.clone())`, `main_body = None`.
     - When both exist with matching hashes: keep root item unchanged
       (`worktree_source = None`, `main_body = None`) — no marker, no diff.
     - When both exist with divergent hashes:
       `merge_main_frontmatter_with_worktree_body` already keeps main
       frontmatter and uses worktree body. Extend it to also populate
       `main_body = Some(<root body>)` and `worktree_source = Some(<wt path>)`.

3. **List marker + preview diff rendering (UI layer)** — `src/ui/mod.rs`.
   - In `render_task_list` (around line 418), when `item.worktree_source.is_some()`,
     prefix the title cell with a single-character marker (proposed: `⎇ `,
     mirroring the existing stage worktree glyph in `src/ui/graph.rs`). The
     marker style uses the same `Modifier::DIM` palette already in use for
     supplementary spans.
   - In `render_preview` (around line 556), when `item.main_body.is_some()`,
     render a unified-diff view of `main_body` (old) vs `item.body` (new)
     instead of the plain markdown body. Use a small pure-function helper
     `render_diff_lines(old: &str, new: &str, width: usize) -> Vec<Line>` that
     wraps the `similar` crate's `TextDiff::from_lines(...).unified_diff()`
     output into ratatui `Line`s with conventional colors
     (`+` green / `-` red / context dim). Place this helper in a new
     `src/ui/diff.rs` module so it is unit-testable without a terminal frame.
   - Add `similar = "2"` to `[dependencies]` in `Cargo.toml`. `similar` is the
     maintained, widely-used diff crate (used by insta, cargo, ratatui's own
     examples); no terminal coupling.

This separation keeps parser/state work in pure modules (`parser`, `domain`),
isolates the diff renderer in a new pure helper (`ui::diff`), and confines TUI
wiring to two existing render functions.

### Focused tests per AC

| AC   | Test location | Test name (proposed) | Verifies |
|------|---------------|----------------------|----------|
| AC-1 | `src/parser/tests.rs` (extend `worktree_only_items_shown`) + new `src/ui/mod.rs` test | `worktree_only_item_has_worktree_source_tag` + `task_row_renders_worktree_marker_when_sourced_from_worktree` | merged item carries `worktree_source = Some(...)` and rendered task row contains the `⎇` marker on that row but not on root-sourced rows. |
| AC-2 | `src/parser/tests.rs` (existing `worktree_status_from_main` already covers this; extend to also assert `worktree_source.is_some()` and `main_body` populated) | `worktree_divergent_keeps_main_frontmatter_and_records_main_body` | merged item's `status`/`title` come from root frontmatter while `body == worktree body` and `main_body == Some(root body)`. |
| AC-3 | `src/ui/diff.rs` (new) + `src/ui/mod.rs` | `render_diff_lines_emits_unified_hunks_with_add_remove_styling` + `preview_renders_diff_when_main_body_present` + `preview_falls_back_to_body_when_main_body_none` | diff helper produces `+`/`-` lines for divergent inputs; preview rendering uses diff path only when `main_body.is_some()`. |
| AC-4 | `src/parser/tests.rs` | `worktree_scan_does_not_mutate_files` | snapshot mtimes of every file in root + worktree fixtures, run `load_workflow_dir`, assert mtimes unchanged and no new files exist. |
| AC-5 | `src/parser/tests.rs` | `worktree_scan_handles_(a)_no_worktree_dirs_(b)_missing_workflow_subdir_(c)_partial_overlap` | three sub-cases per the AC; each asserts `Ok(_)` and that the merged item list equals the root-only baseline (no spurious entries, no errors). Add a `claude_worktrees_dir_is_scanned_alongside_dot_worktrees` test asserting both conventions are picked up. |

### Verification commands

The implementer must run, in order, before marking the implement stage complete:

1. `cargo test` — all unit + integration tests pass (new tests above + existing
   suite unchanged).
2. `make lint` — `cargo clippy --all-targets --all-features -- -D warnings`
   passes with zero diagnostics. New code must not introduce `#[allow(...)]`.
3. `cargo run -- -w docs/spacetop-dev` (smoke) — open the live workflow in a
   checkout that has at least one `.worktrees/<slug>/` mirror and visually
   confirm the marker appears and the preview shows a diff. (Smoke step is
   recommended, not gating; tests cover the contract.)

### Read-only invariant preservation

Explicit guardrails the implementer must hold:

- No new code path writes to disk. The merge function operates on
  already-parsed `WorkItem` values and an in-memory `HashMap`.
- The diff renderer reads `item.main_body` and `item.body` (both already
  in-memory strings); it never re-reads files.
- The expanded worktree enumeration only adds `fs::read_dir` calls on
  `.claude/worktrees`; no `canonicalize` writes, no symlink creation, no
  temp-file usage.
- AC-4 is enforced by an explicit mtime-stability test (see table above), not
  only by code review.

## Design choices (implement stage)

- **Worktree marker glyph.** Worktree-sourced task rows are distinguished by a
  leading `⎇` (U+2387) glyph rendered with the DIM style. The glyph occupies a
  fixed 2-column slot between the id column and the title so titles stay
  aligned across all rows. The glyph already appears in `src/ui/graph.rs` for
  stages marked `worktree: true`, so reusing it keeps the visual vocabulary
  consistent across the UI. The existing `task_row_no_glyphs_in_phase_col`
  test continues to pass because the marker sits *after* the phase column.

## Stage Report: plan

- DONE: Plan separates worktree discovery, parser/merge logic, and preview-diff rendering into distinct testable units, naming the module/file each piece lands in. — Three units called out under "Testable units & file ownership": parser layer (`src/parser/worktree.rs`), domain+merge (`src/domain/mod.rs` + `src/parser/worktree.rs`), and UI (`src/ui/mod.rs` + new `src/ui/diff.rs`).
- DONE: Plan specifies the verification commands the implementer will run (at minimum `cargo test` and `make lint`) and names the focused tests it expects to add for AC-1 through AC-5. — "Verification commands" lists `cargo test`, `make lint`, and an optional smoke run; "Focused tests per AC" table maps each AC to a named test in a specific file.
- DONE: Plan addresses how to enumerate worktree workflow copies (e.g., `git worktree list --porcelain` vs scanning `.worktrees/`/`.claude/worktrees/`) and explicitly preserves the read-only invariant. — "Worktree enumeration approach" picks filesystem scanning of both `.worktrees/*` and `.claude/worktrees/*` with rationale; "Read-only invariant preservation" section enumerates the four guardrails plus an mtime-stability test.

## Stage Report: implement

- DONE: AC-1 — Worktree-only tasks appear in the task list with a distinguishing marker. Verified by `parser::tests::worktree_only_item_has_worktree_source_tag` (merge tags `worktree_source = Some(wt_path)`), `parser::tests::claude_worktrees_dir_is_scanned_alongside_dot_worktrees` (both `.worktrees/*` and `.claude/worktrees/*` are scanned), and `ui::tests::task_row_renders_worktree_marker_when_sourced_from_worktree` (rendered task row shows the `⎇` glyph on worktree-sourced rows and not on main-only rows). Files touched: `src/domain/mod.rs`, `src/parser/worktree.rs`, `src/parser/item.rs`, `src/ui/mod.rs`.
- DONE: AC-2 — When a task exists in both root and worktree, status uses the root copy and `main_body` retains the root body. Verified by `parser::tests::worktree_divergent_keeps_main_frontmatter_and_records_main_body` (asserts merged item's `status`/`title` are from main, `body` is from worktree, `main_body == Some(<root body>)`, and `worktree_source` is populated). Files touched: `src/parser/worktree.rs`.
- DONE: AC-3 — Preview shows a body diff when root and worktree copies differ. Verified by `ui::diff::tests::render_diff_lines_emits_unified_hunks_with_add_remove_styling` (helper produces `+`/`-` lines with green/red styling), `ui::diff::tests::render_diff_lines_identical_bodies_emits_only_context` (identical bodies yield only context lines), `ui::tests::preview_renders_diff_when_main_body_present` (rendered preview buffer contains `+NEW LINE` / `-OLD LINE`), and `ui::tests::preview_falls_back_to_body_when_main_body_none` (no diff prefixes when `main_body` is None). Files touched: new `src/ui/diff.rs`, `src/ui/mod.rs`, `Cargo.toml` (added `similar = "2"`).
- DONE: AC-4 — Worktree scanning is read-only and does not mutate workflow files. Verified by `parser::tests::worktree_scan_does_not_mutate_files` (snapshots every file's path, size, and mtime across root + `.worktrees` + `.claude/worktrees`, runs `load_workflow_dir`, and asserts no file is created, deleted, resized, or has its mtime changed). The implementation only calls `fs::read_dir` and `fs::read_to_string`/`fs::read`; no write/create/remove paths were added. Files touched: `src/parser/worktree.rs`.
- DONE: AC-5 — Worktree discovery handles absent worktree directories, missing per-worktree workflow paths, and unrelated subsets gracefully. Verified by `parser::tests::worktree_scan_handles_no_worktrees_missing_subdir_and_partial_overlap` (three sub-cases: no worktree dirs, worktree without the workflow subdir, and worktree with non-overlapping task files; each returns `Ok` and the merged list matches the root-only baseline plus any genuinely new worktree items). Files touched: `src/parser/worktree.rs`.
- DONE: TUI list visibly distinguishes worktree-sourced rows. The `⎇` (U+2387) glyph, DIM-styled, is inserted after the id column and before the title in `render_task_list` (`src/ui/mod.rs:533`). The placement keeps row alignment intact and stays outside the phase column so `ui::tests::task_row_no_glyphs_in_phase_col` continues to pass. Rationale is recorded under "Design choices (implement stage)" in this entity body.
- DONE: Preview shows a body diff when root and worktree copies differ. `render_preview` (`src/ui/mod.rs:556`) consults `item.main_body`; when populated, it renders via `diff::render_diff_lines` (`src/ui/diff.rs`) and skips the markdown pass. When `main_body` is None, the existing single-body markdown rendering is preserved.
- DONE: `make lint` passes — clippy `-D warnings` reported zero diagnostics (run from worktree, exit status 0). No `#[allow(...)]` directives were added.
- DONE: `cargo test --lib` and `cargo test --tests`: all 10 new tests pass (`worktree_only_item_has_worktree_source_tag`, `worktree_divergent_keeps_main_frontmatter_and_records_main_body`, `claude_worktrees_dir_is_scanned_alongside_dot_worktrees`, `worktree_scan_handles_no_worktrees_missing_subdir_and_partial_overlap`, `worktree_scan_does_not_mutate_files`, `render_diff_lines_emits_unified_hunks_with_add_remove_styling`, `render_diff_lines_identical_bodies_emits_only_context`, `task_row_renders_worktree_marker_when_sourced_from_worktree`, `preview_renders_diff_when_main_body_present`, `preview_falls_back_to_body_when_main_body_none`). Pre-existing test `ui::graph::tests::narrow_tier_renders_compact_textual_summary` continues to fail on the baseline branch (verified by stashing this branch's changes); it is unrelated to this task and is left for a separate fix. Final tally: 201 passed; 1 pre-existing failure; 0 ignored.
- DONE: Read-only invariant preserved — no writes were added to either root or worktree workflow directories outside the entity file's stage report. AC-4's mtime-stability test exercises this explicitly.

## Stage Report: review

- DONE: AC-1 PASS — `parser::tests::worktree_only_item_has_worktree_source_tag` (src/parser/tests.rs:756) asserts merged worktree-only item carries `worktree_source = Some(wt_path)`; `parser::tests::claude_worktrees_dir_is_scanned_alongside_dot_worktrees` (src/parser/tests.rs:834) verifies both `.worktrees/*` and `.claude/worktrees/*` are scanned; `ui::tests::task_row_renders_worktree_marker_when_sourced_from_worktree` (src/ui/mod.rs:3330) asserts the `⎇` glyph appears on worktree rows and not on main rows. All three observed passing.
- DONE: AC-2 PASS — `parser::tests::worktree_divergent_keeps_main_frontmatter_and_records_main_body` (src/parser/tests.rs:782) asserts merged item's `status`/`title` come from main frontmatter, `body` is from worktree, `main_body == Some(<root body>)`, and `worktree_source` is populated. Observed passing.
- DONE: AC-3 PASS — `ui::diff::tests::render_diff_lines_emits_unified_hunks_with_add_remove_styling` (src/ui/diff.rs:39) verifies `+`/`-` lines with green/red styling; `ui::diff::tests::render_diff_lines_identical_bodies_emits_only_context` (src/ui/diff.rs:70) covers identical inputs; `ui::tests::preview_renders_diff_when_main_body_present` (src/ui/mod.rs:3374) and `ui::tests::preview_falls_back_to_body_when_main_body_none` (src/ui/mod.rs:3396) cover preview path selection. All four observed passing.
- DONE: AC-4 PASS — `parser::tests::worktree_scan_does_not_mutate_files` (src/parser/tests.rs:903) snapshots paths, sizes, and mtimes across root + `.worktrees` + `.claude/worktrees`, runs `load_workflow_dir`, and asserts no file is created, deleted, resized, or has its mtime changed. Confirmed no `fs::write`, `File::create`, `OpenOptions`, `fs::remove*`, or `fs::create_dir*` calls were introduced in any of the modified production modules (src/parser/worktree.rs, src/parser/item.rs, src/domain/mod.rs, src/discovery.rs, src/ui/mod.rs, src/ui/diff.rs, src/ui/graph.rs, src/app/overview.rs). Existing fs::write hits in src/lib.rs, src/discovery.rs (test scaffolding), and src/app/tests.rs are pre-existing test setup, untouched by this diff.
- DONE: Crate hygiene — `similar = "2"` added to `[dependencies]` in `Cargo.toml`. Crate is widely-used (insta, cargo-deny, ratatui examples), maintained, and pinned to a major version which is conventional for Rust. Use is confined to the new pure helper `src/ui/diff.rs`; no terminal coupling.
- DONE: `cargo test` — Run from the worktree: 201 passed; 1 failed (`ui::graph::tests::narrow_tier_renders_compact_textual_summary`); 0 ignored. Verified the failing test predates this branch: the diff for `src/ui/graph.rs` only adds two new field initializers (`worktree_source: None`, `main_body: None`) inside an unrelated test helper near line 1195; the failing test body at line 849 is unchanged. Exit status: failure (due to pre-existing failure). The pre-existing failure is documented and is unrelated to this task.
- DONE: `make lint` — `cargo clippy --all-targets --all-features -- -D warnings` exit status 0 with zero diagnostics. No `#[allow(...)]` directives introduced.
- DONE: TUI behavior soundness — Marker placement is after the id and phase column and before the title (src/ui/mod.rs render_task_list), preserving alignment; the existing `task_row_no_glyphs_in_phase_col` test still passes. Preview diff rendering switches on `main_body.is_some()` so the existing single-body markdown path is preserved when there is no divergence, and the `preview_falls_back_to_body_when_main_body_none` test pins that behavior. No regressions observed in existing ui snapshot/render tests.

Verdict: PASSED
