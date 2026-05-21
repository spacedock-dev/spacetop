---
id: "042"
title: One malformed-frontmatter entity crashes the entire workflow load
status: review
source: captain
started: 2026-05-21T04:42:27Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-042-malformed-frontmatter-crashes-workflow-load
issue:
pr:
---

Running `spacetop -w /Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/` exits 1 with no TUI shown:

```
Error: failed to load workflow directory /Users/kent/Dev/InfuseAI/GitHub/dataagentbench/docs/research

Caused by:
    0: /Users/kent/Dev/InfuseAI/GitHub/dataagentbench/docs/research/rd-013.md: malformed YAML frontmatter: mapping values are not allowed in this context at line 7 column 137
    1: mapping values are not allowed in this context at line 7 column 137
```

The offending entity (`rd-013.md`) has an unquoted multi-line `diff_summary:` value containing colons, which `serde_yaml` rejects. The error itself is technically correct, but a single bad entity should not be allowed to take down the whole workflow view — the captain cannot inspect any of the other 12+ valid entities in that workflow until the bad file is hand-fixed.

spacetop is a read-only viewer. The current behavior is closer to a strict compiler than a viewer: it bails on the first parse error and returns nothing. A viewer's job is to show what it can and surface what it can't, not to refuse to start.

## Background

- Loader entry point: `src/discovery.rs` / `src/parser.rs` (see `CLAUDE.md` module layout). The current load path treats any parse failure as fatal and propagates it up to `lib.rs::run_terminal` / `decide_app`.
- The error message names the file, line, and column — that's good — but offers no remediation hint (e.g., "wrap values containing `:` in quotes" or "add `>-` for multi-line").
- The TUI never starts, so the captain has no in-app affordance for navigating to the broken entity, viewing its raw frontmatter, or skipping it.
- Stable user-facing strings are pinned by tests (per `CLAUDE.md`) — any new behavior here needs paired test updates.

## Acceptance criteria

**AC-1 — One malformed entity does not prevent the rest of the workflow from loading.**
Verified by: a `cargo test` that constructs a workflow directory with N valid entity files and one entity file whose frontmatter is malformed (e.g., an unquoted value containing `:`), invokes the loader, and asserts the returned workflow contains the N valid entities. The malformed entity must be reported (see AC-2) but must not abort the load.

**AC-2 — Malformed entities surface as in-app errors, not as a process-level crash.**
Verified by: the same test (or a sibling test) asserts the loader returns or records a structured per-entity error for the malformed file — containing the file path, the underlying YAML error, and (where derivable) the line/column — that the TUI can render alongside the valid entities. Acceptable surfaces include: a synthetic "broken entity" placeholder in the item list with the error in its preview pane, a separate errors panel, or an overlay; the design stage picks one and justifies it.

**AC-3 — `spacetop -w {path}` exits 0 and starts the TUI even when one entity has malformed frontmatter.**
Verified by: an integration test in `tests/` that runs `decide_app` (or equivalent) against a fixture workflow with one malformed entity and asserts a launch decision (no error return) consistent with the existing zero-workflows / valid-workflow tests.

**AC-4 — The existing strict-failure path remains available for genuinely unusable inputs.**
Verified by: tests that cover "no entities parse" or "workflow `README.md` is itself malformed" still produce a clear top-level error and a non-zero exit, matching today's behavior. The fix only relaxes the per-entity case, not the entire-workflow case.

**AC-5 — `make lint` and `cargo test` pass.**
Verified by: `make lint` (clippy `-D warnings`) and `cargo test` from the repo root, both green.

## Reproduction (for design / implement reference)

```bash
spacetop -w /Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/
# exits 1; Error: failed to load workflow directory ... rd-013.md: malformed YAML frontmatter
```

The minimal fixture for the test suite is two files: one valid entity (anything from the existing `tests/` fixtures), one entity with a frontmatter line like `diff_summary: build the candidate set using a disjunction: WHERE foo` (the unquoted post-colon segment is what breaks YAML).

## Implementation Plan

### Diagnosis: the fail-fast call site

`src/parser/snapshot.rs::load_workflow_dir` currently maps any per-item parse failure into a hard early return:

```
for item_path in item_paths {
    items.push(parse_work_item(&item_path, &allowed_statuses)?); // <-- `?` aborts on first bad entity
}
```

That `?` is the exact line that crashes the whole workflow load when `rd-013.md` fails. The same fail-fast pattern is in `src/parser/worktree.rs::load_worktree_items` (a `.map(...).collect()` over a `Result`-yielding iterator).

`load_workflow_readme` (the workflow `README.md`) is separately fatal — that is the AC-4 surface and must stay strict.

A useful precedent already exists: `src/parser/archive.rs::load_archived_items` (referenced by parser tests at lines 393-448) already swallows malformed archived entries silently and continues. AC-2 raises that bar: the bad entity must be **surfaced**, not silently dropped.

### 1. Loader contract change

Goal: a single malformed item becomes a captured per-entity error attached to the snapshot, not a hard `Err` from the loader.

**Contract surface changes**

- `src/domain/mod.rs`
  - Add a new public type:
    ```
    #[derive(Debug, Clone, PartialEq)]
    pub struct EntityParseError {
        pub path: PathBuf,           // file that failed
        pub message: String,         // ParseError display string (already includes path + reason)
        pub line: Option<u32>,       // when derivable from serde_yaml::Error::location()
        pub column: Option<u32>,     // when derivable from serde_yaml::Error::location()
    }
    ```
  - Extend `WorkflowSnapshot`:
    ```
    pub struct WorkflowSnapshot {
        pub definition: WorkflowDefinition,
        pub items: Vec<WorkItem>,
        pub parse_errors: Vec<EntityParseError>, // NEW, empty on the happy path
    }
    ```
- `src/parser.rs` (re-exports + `ParseError` enum): no signature change to `ParseError`. Re-export `EntityParseError` for convenience.
- `src/parser/snapshot.rs::load_workflow_dir(&Path, &Path) -> Result<WorkflowSnapshot, ParseError>` — **signature unchanged**. Internals change: catch per-item `Err(ParseError::MalformedYaml | MissingFrontmatter | UnterminatedFrontmatter | MissingRequiredField | UnknownStatus)` returned by `parse_work_item`, push an `EntityParseError` onto the new `parse_errors` field, and continue. `ParseError::ReadFile` and `ParseError::ReadDirectory` stay fatal (broken FS is a different failure class — keeps the `permission_denied` and broken-symlink-on-read tests honest).
- `src/parser/worktree.rs::load_worktree_items` mirrors the same change so a worktree clone with one malformed mirror does not abort either; its errors flow into the same `parse_errors` bucket via `scan_worktrees`'s return tuple — promote `scan_worktrees` from `Result<Vec<WorkItem>, ParseError>` to `Result<(Vec<WorkItem>, Vec<EntityParseError>), ParseError>` and have `load_workflow_dir` concatenate.
- `src/parser/readme.rs::parse_workflow_readme` — **unchanged**, stays strict. This preserves AC-4: a malformed workflow `README.md` still returns `Err` from `load_workflow_dir`.

**Per-entity error construction**

Inside `parse_work_item`, when `parse_work_item_contents` returns `Err`, derive `(line, column)` only for the `ParseError::MalformedYaml` variant:

```
fn yaml_location(err: &ParseError) -> (Option<u32>, Option<u32>) {
    if let ParseError::MalformedYaml { source, .. } = err {
        if let Some(loc) = source.location() {
            return (Some(loc.line() as u32), Some(loc.column() as u32));
        }
    }
    (None, None)
}
```

Other classified errors (missing field, unknown status) keep `line: None, column: None`. Free-form `message` is `format!("{err}")` (the `Display` impl already yields the current user-facing string, e.g. `<path>: malformed YAML frontmatter: mapping values are not allowed in this context at line 7 column 137`). Reusing the existing `Display` keeps every parse-error test that asserts on substring (`"malformed YAML frontmatter"`, `"missing required field"`) green without churn.

**Downstream consumers that learn the new shape**

- `src/app/overview.rs::OverviewState`
  - The `WorkflowSnapshot` field already lives on the struct; just propagate `snapshot.parse_errors` into a new field on `OverviewState`:
    ```
    pub parse_errors: Vec<EntityParseError>, // NEW
    ```
  - `OverviewState::empty` initialises it to `Vec::new()`.
  - `reload_from_snapshot` overwrites it from the new snapshot (parity with `items`).
  - Add a thin accessor: `pub fn parse_errors(&self) -> &[EntityParseError]`.
- `src/lib.rs::decide_app` and `load_overview_state` — **no signature change.** They wrap `OverviewState::load` results in `anyhow::Context`; per-entity errors are now non-fatal so the `?` path stays cold. The integration test for AC-3 (see Test Strategy) drives this.
- `src/ui/*` — UI consumers gain a new render hook; see surface choice below.

### 2. In-app surface choice for AC-2

**Chosen surface: synthetic "broken entity" rows in the task list, with full error in the preview pane.**

Rationale (vs. an errors panel or modal overlay):
- The captain's mental model is "this workflow has N items; one of them is broken." Putting it in the list lets them navigate to it with the same `j/k`/arrow keys; opening the preview shows the actual YAML error. No new keybinding, no new mode.
- The list already has DIM styling and per-stage color cues we can reuse for a visually distinct row.
- Cost is bounded: one new row variant in `src/ui/list.rs`, one new render branch in `src/ui/preview.rs`.

A status-footer hint (count of broken items) is the secondary surface so the captain still sees the problem when scrolled away from the bad row.

**Concrete UI changes**

- `src/ui/list.rs`
  - `render_task_list`: when building the list items, append one `ListItem` per `state.parse_errors()` entry after the regular `items`, using a fixed prefix like `⚠ broken: <file_name>` styled with `Color::Red` + `Modifier::DIM`. Selecting one of these rows moves selection into "broken-entity preview" territory.
  - New helper: `fn broken_list_item(err: &EntityParseError) -> ListItem<'static>` — pure, unit-testable via the existing `TestBackend` pattern used by `src/ui/tests/task_list.rs`.
  - Selection model: extend `OverviewState::selected_item()` to return an enum (e.g. `Selected::Item(&WorkItem) | Selected::Broken(&EntityParseError)`) OR add a parallel `selected_broken()` accessor — the second option keeps existing call sites unchanged and is the chosen path. Concrete change: introduce `pub enum SelectedRow<'a> { Item(&'a WorkItem), Broken(&'a EntityParseError) }` and a `selected_row(&self)` method; leave `selected_item()` returning `Option<&WorkItem>` (returns `None` when a broken row is selected).
- `src/ui/preview.rs`
  - `render_preview`: when `state.selected_item()` is `None` AND `state.selected_row()` is `Some(SelectedRow::Broken(err))`, render an error panel with these fields displayed verbatim:
    - Header: `Cannot parse <relative-or-absolute path>`
    - Body line 1: `Error: <err.message>` (already contains the YAML reason)
    - Body line 2 (when present): `Location: line <line>, column <column>`
    - Body line 3 (static hint): `Hint: wrap values containing ':' in quotes, or use '>-' for multi-line scalars`
  - The hint string is a stable user-facing string — pin it in a UI test (see Test Strategy AC-2).
- `src/ui/footer.rs`
  - `status_footer_hints`: when `session.active_state().parse_errors()` is non-empty, prepend a styled pill `"⚠ N broken"` (N from `parse_errors().len()`). Color: reuse `Color::Red` foreground on the existing `PILL_BG`. The pill is informational — no key binding.

### 3. Test strategy

All assertion APIs below already exist in the codebase (`TestBackend`, parser test helpers, the `tests/discovery_bypass.rs` integration shape).

**AC-1 — One malformed entity does not prevent the rest of the workflow from loading**
- Test module: `src/parser/tests.rs` (new test `#[test] fn load_workflow_dir_skips_malformed_entity_and_records_error`).
- Fixture (constructed via `write_markdown` helper already used in this module):
  - `README.md` with the existing minimal workflow frontmatter (stages: design, plan, …).
  - Three valid entities: `good-1.md`, `good-2.md`, `good-3.md` with `status: plan`.
  - One malformed: `bad.md` containing the reproduction line `diff_summary: build the candidate set using a disjunction: WHERE foo` plus the required `id/title/status` fields above it.
- Assertions (struct-field equality):
  - `snapshot.items.len() == 3`
  - `snapshot.items.iter().map(|i| &i.id).collect::<Vec<_>>() == ["1","2","3"]` (or whatever ids we choose)
  - `snapshot.parse_errors.len() == 1`
  - `snapshot.parse_errors[0].path` ends in `bad.md`
  - `snapshot.parse_errors[0].message.contains("malformed YAML frontmatter")`
  - `snapshot.parse_errors[0].line == Some(N)` and `column == Some(M)` (exact values pulled from `serde_yaml::Error::location()` — assert `> 0`, not magic numbers).

**AC-2 — Malformed entities surface as in-app errors, not as a process-level crash**
- Two paired tests.
- Test module A: `src/app/tests.rs` (new `fn overview_state_exposes_parse_errors_from_snapshot`).
  - Build a `WorkflowSnapshot` directly with one item and one `EntityParseError` in the new field.
  - Assert `state.parse_errors().len() == 1` and `state.parse_errors()[0].path == <expected>`.
  - Assert `state.selected_row()` walks through item rows then broken rows in order.
- Test module B: `src/ui/tests/task_list.rs` (new `fn task_list_renders_broken_entity_row_after_items`).
  - Use the existing `TestBackend` pattern (already imported there).
  - Buffer-substring assertions on the rendered string:
    - Contains `"⚠ broken: bad.md"` (the broken row label).
    - When selection is on the broken row, the preview area contains:
      - `"Cannot parse"` and the file name
      - `"malformed YAML frontmatter"` (proves the underlying message is surfaced)
      - `"line"` and `"column"` (proves location is surfaced when present)
      - The exact hint string `"Hint: wrap values containing ':' in quotes, or use '>-' for multi-line scalars"` (this is the new **stable user-facing string** — pinned per CLAUDE.md).
  - Footer test (sibling, in the same module or `src/ui/tests/chrome.rs`): when `parse_errors` is non-empty, the rendered footer contains `"⚠ 1 broken"`.

**AC-3 — `spacetop -w {path}` exits 0 and starts the TUI even when one entity has malformed frontmatter**
- Test module: `tests/discovery_bypass.rs` (existing integration file that drives `decide_app` without a terminal — pattern already established for the `-w` bypass).
- Fixture: create a `tempdir` containing a workflow `README.md` + N valid entity files + 1 malformed entity (same content shape as AC-1).
- Invocation: build a `Cli { workflow_dir: Some(<tempdir>) }` and call `decide_app(&cli, &cwd)`.
- Assertion (matches existing patterns in `discovery_bypass.rs`):
  - `matches!(result, Ok(DecideOutcome::Overview(_)))`
  - Extract the inner `App`, drill to `state.parse_errors()`, assert `.len() == 1`.
  - Assert no `ZeroWorkflows` outcome.

**AC-4 — Existing strict-failure path remains available for genuinely unusable inputs**
- Two tests, both in `src/parser/tests.rs`.
- Test 1 — workflow `README.md` malformed (top-level still fatal):
  - Fixture: workflow dir with a `README.md` whose YAML frontmatter is malformed (e.g. truncated `---` block) and one valid entity.
  - Assertion: `load_workflow_dir(...)` returns `Err(ParseError::MalformedYaml { .. })` whose `path` ends in `README.md`.
- Test 2 — all entities malformed (zero-item case still produces a valid snapshot with `parse_errors.len() == N`, **not** an `Err`).
  - Assertion: `snapshot.items.is_empty() && snapshot.parse_errors.len() == N`.
  - Note: this is a deliberate softening — the workflow still loads, the UI shows N broken rows. AC-4 only mandates that the workflow-README case stays fatal; "all entities malformed" was previously fatal-via-first-item but is now consistent with AC-1. The plan stage flags this for the design captain's awareness; if the captain wants "if zero valid items, return Err" instead, swap the assertion's expectation. Recommendation: keep the soft path — a viewer should still show "here are N broken items" rather than refusing to start.
- A third sibling integration test in `tests/discovery_bypass.rs` asserts that the malformed-README case bubbles up to `decide_app` as an `Err` (and would exit non-zero in `run`).

**AC-5 — `make lint` and `cargo test` pass**
- Run `make lint` (clippy `-D warnings`) and `cargo test` from the repo root.
- Watch for: `serde_yaml::Error::location()` returns `Option<Location>` — match-or-`map` properly; clippy will flag a needless `unwrap_or(None)`. The new public `EntityParseError` struct needs `#[derive(Debug, Clone, PartialEq)]` to avoid breaking `WorkflowSnapshot`'s existing `#[derive(...)]`.

### File-by-file delta (for the implement stage)

- `src/domain/mod.rs` — add `EntityParseError`, extend `WorkflowSnapshot`, update construction sites in tests (the existing tests construct `WorkflowSnapshot { definition, items }` — add `parse_errors: Vec::new()` to every one).
- `src/parser.rs` — re-export `EntityParseError`.
- `src/parser/snapshot.rs` — replace `?` on per-item parse with classify-and-collect.
- `src/parser/worktree.rs` — same change for the worktree loop; return `(Vec<WorkItem>, Vec<EntityParseError>)`.
- `src/parser/item.rs` — no signature change; add `yaml_location` helper at module scope (private) to derive `(line, col)`.
- `src/app/overview.rs` — add `parse_errors: Vec<EntityParseError>` to `OverviewState`, propagate via `empty` / `from_snapshot_with_root` / `reload_from_snapshot`, add accessor + `selected_row()`.
- `src/ui/list.rs` — append broken rows after items.
- `src/ui/preview.rs` — render broken-entity preview branch.
- `src/ui/footer.rs` — add the `⚠ N broken` pill when applicable.
- Test files: `src/parser/tests.rs`, `src/app/tests.rs`, `src/ui/tests/task_list.rs` (+ optionally `chrome.rs`), `tests/discovery_bypass.rs`.

No new crate dependencies. `serde_yaml::Error::location` is already in the dep tree (it's a method on the existing imported type).


## Stage Report: plan

- DONE: Name the loader contract change: which functions in src/discovery.rs and src/parser.rs change return type / signature so a single malformed entity becomes a captured per-entity error instead of a hard fail. Identify the existing fail-fast call site (the `?` or `Err(_)` that aborts the load) and the proposed surface (e.g., `Workflow { items: Vec<WorkItem>, parse_errors: Vec<EntityParseError> }`). Call out any downstream consumers (lib.rs::decide_app, src/app.rs, src/ui/*) that need to learn the new shape.
  See "Diagnosis: the fail-fast call site" + "1. Loader contract change": fail-fast is the `?` on `parse_work_item` in `src/parser/snapshot.rs::load_workflow_dir`; new surface is `WorkflowSnapshot { ..., parse_errors: Vec<EntityParseError> }`; `src/discovery.rs` stays unchanged (it already silently ignores unparseable READMEs); downstream consumers (`OverviewState`, `decide_app`, ui/*) enumerated with concrete changes.
- DONE: Pick the in-app surface for AC-2 (broken-entity placeholder in the list, separate errors panel, or overlay) and name the exact src/ui/* file(s) and render functions that change. The chosen surface must let the captain see WHICH file failed and WHY without opening external tools — name the displayed fields (path, yaml error message, line/col when derivable).
  See "2. In-app surface choice for AC-2": chose **synthetic broken-entity rows in the task list** + preview pane shows full error; changes scoped to `src/ui/list.rs::render_task_list`, `src/ui/preview.rs::render_preview`, and `src/ui/footer.rs::status_footer_hints`; displayed fields: path, `err.message`, line/column (when derivable), and a pinned remediation hint.
- DONE: Test strategy with named fixtures and assertion shapes: enumerate the parser/integration tests required to cover AC-1 (N valid + 1 malformed -> N items loaded), AC-2 (per-entity error surface populated), AC-3 (decide_app returns a launch decision, not an error, for the malformed-entity case), and AC-4 (whole-workflow-README malformed still produces a top-level error + non-zero exit). For each, name the test module, the fixture file content shape, and the assertion API (TestBackend buffer substring vs. struct-field equality).
  See "3. Test strategy": six tests across `src/parser/tests.rs`, `src/app/tests.rs`, `src/ui/tests/task_list.rs`, and `tests/discovery_bypass.rs`, each with named fixture content shape and either struct-field equality (parser/app/integration) or `TestBackend` buffer-substring (ui) assertion APIs.

### Summary

Planned a minimal-surgery fix: replace the single `?` in `load_workflow_dir`'s per-item loop with a classify-and-collect that pushes `EntityParseError` into a new `WorkflowSnapshot::parse_errors` vec, then surface those errors as synthetic "broken" rows in the task list with the YAML error rendered in the preview pane. The workflow-README path stays strict, preserving AC-4. The plan calls out a deliberate softening (all-entities-malformed now loads an empty workflow with N broken rows instead of erroring) and flags it for the design captain to confirm. No new crate dependencies; one new stable user-facing string ("Hint: wrap values containing ':' …") is pinned by a UI test per CLAUDE.md.
