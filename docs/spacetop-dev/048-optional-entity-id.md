---
id: "048"
title: "Support optional entity ID (id-style: slug)"
status: review
source: captain
started: 2026-06-09T01:20:12Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-048-optional-entity-id
issue:
pr:
mod-block: merge:pr-merge
---

The newer Spacedock README format declares `id-style: slug` in the workflow frontmatter, which means entity files carry a blank `id:` field — identity comes from the filename slug instead of a numeric or sd-b32 value. SpaceTop currently expects a populated `id` field on every entity; it should handle the slug-identity case gracefully and display the slug as the effective ID.

Reference workflow: `/Users/kent/Dev/InfuseAI/paper-work/roadmap-playground/docs/roadmap-workflow`  
Example entities: `roadmap-v5.md`, `adversarial-review.md` (both have `id:` blank, `id-style: slug` declared in README).

## Acceptance criteria

**AC-1 — No crash on blank `id:` field.**  
Verified by: `cargo test` passes against a fixture workflow that uses `id-style: slug` with entities whose `id:` is empty or absent.

**AC-2 — Slug used as display ID when `id:` is blank.**  
Verified by: the status table (overview UI) shows the entity slug in the ID column when `id:` is blank, confirmed by an integration test or a snapshot of terminal output.

**AC-3 — `id-style` read from README frontmatter.**  
Verified by: `parser.rs` (or its domain model) exposes `id_style` from the workflow README, and the test fixture exercises both `sequential` and `slug` values.

**AC-4 — Existing sequential-id workflows unaffected.**  
Verified by: all existing tests still pass; no regression in the spacetop-dev workflow's own numeric IDs.

## Implementation Plan

### Current state (grounding)

- `src/parser/item.rs:29` — `let id = required(raw.id, path, "id")?;` is the single point that rejects a blank/absent `id:`. `required()` (`src/parser/parser.rs:110`) errors with `MissingRequiredField` when the value is empty after trim. This is exactly the crash path AC-1 targets.
- `src/parser/item.rs:11` — `parse_work_item(path, _allowed_statuses)` does **not** receive `id_style`. Its only caller is `src/parser/snapshot.rs:23` (`parse_work_item(&item_path, &allowed_statuses)`), which **does** have `definition.id_style` in scope. So the id-style decision can be threaded through the loader without touching the TUI.
- `src/parser/readme.rs:69` — `id_style: raw.id_style` is already parsed from README frontmatter into `WorkflowDefinition.id_style: Option<String>` (`src/domain/mod.rs:89`). AC-3 is therefore already satisfied at the parse layer for *reading*; what's missing is a fixture exercising both `sequential` and `slug` and a test asserting the field round-trips.
- `src/ui/list.rs:136` — the ID column is `format!("{:>4}", item.id)`. AC-2 only requires `item.id` to *hold* the slug; the existing renderer then displays it. No new rendering branch is needed — the fix is upstream, in what value lands in `WorkItem.id`.
- `src/ui/definition.rs:90` — `id_style` is already surfaced in the scope subline; no change needed.

### Design: where the slug comes from

When `id-style: slug` and the entity's `id:` is blank/absent, the **effective ID is the filename stem** (`roadmap-v5.md` → `roadmap-v5`, matching the reference workflow). The slug is derived from `path.file_stem()`. Folder-form entities (`{slug}/index.md`) are out of scope for active items here (active items are flat `.md` files per `collect_active_item_paths`); if `index` ever appears as a stem it is acceptable for this slice — note it, do not special-case it.

### Step-by-step (parser/state work — no terminal)

1. **Thread `id_style` into work-item parsing.** Change `parse_work_item` to take the workflow's id-style. Two viable shapes — pick the first:
   - **Preferred:** add a parameter `id_style: Option<&str>` to `parse_work_item` and `parse_work_item_contents`. Update the only caller (`snapshot.rs:23`) to pass `definition.id_style.as_deref()`. Keeps the signature explicit and the change local.
   - Alternative (avoid): resolving the slug inside `snapshot.rs` after the fact — rejected because it splits the id-resolution logic across two modules and complicates the per-entity error path.

2. **Make `id` optional when style is slug.** In `parse_work_item_contents` (`item.rs:~29`), replace the unconditional `required(raw.id, path, "id")?` with a resolution helper:
   - If `optional_text(raw.id)` is `Some(v)` → use `v` (covers AC-4: sequential workflows with populated ids, and slug workflows that happen to fill `id:`).
   - Else if `id_style == Some("slug")` → derive slug from `path.file_stem()` (lossy-to-`str`, fall back to the file name). Use that as `id`.
   - Else → keep the current `MissingRequiredField{ field: "id" }` error (a non-slug workflow with a blank id is still an error, preserving today's behavior for sequential workflows).
   This keeps the blank-id tolerance scoped to slug workflows only, so AC-4 sequential workflows are untouched.

3. **No domain struct change required.** `WorkItem.id: String` already holds the effective id; we are only changing how it is populated. Do not add an `id_style` field to `WorkItem` — the resolved id is sufficient for the renderer and avoids leaking workflow-level state onto every row.

4. **No TUI change required for AC-2.** `src/ui/list.rs:136` already renders `item.id`. Once `item.id` carries the slug, the overview ID column shows it. (Caveat to record, not fix in this slice: `{:>4}` right-truncates wide slugs visually — slugs longer than 4 chars overflow the 4-col field but are not crashed or dropped. Widening the column is a separate change; flag it in the report, leave it to a follow-up unless the validation stage rules otherwise.)

### Test strategy (focused; proves AC-1, AC-2, AC-3, AC-4)

All new tests live in `src/parser/tests.rs` (parser-level, no terminal) plus one snapshot-level test, matching the existing `#[cfg(test)]` convention there. The file already has both fixture styles: on-disk `fixture_root()` (`CARGO_MANIFEST_DIR/docs/spacetop-dev`) and in-memory `unique_temp_dir`/`write_markdown` helpers (`tests.rs:272,282`).

**New fixture (committed on disk):** add a slug-style fixture workflow under `tests/fixtures/slug-workflow/` (new dir — the *at least one new fixture file* the checklist requires):
- `tests/fixtures/slug-workflow/README.md` — frontmatter with `commissioned-by: spacedock@0.19.8`, `id-style: slug`, and a minimal `stages.states` block whose `name:` values cover the statuses used by the entities (e.g. `done`). Mirror the shape of the real reference README (`/Users/kent/Dev/InfuseAI/paper-work/roadmap-playground/docs/roadmap-workflow/README.md`).
- `tests/fixtures/slug-workflow/roadmap-v5.md` — `id:` blank, `title:`, `status: done` (mirrors the reference entity). This is the slug-identity entity under test.

(Alternatively the same fixtures can be built in a temp dir via the existing `write_markdown` helpers; the committed-fixture form is preferred because the checklist explicitly asks for a fixture *file*, and it documents the supported on-disk shape.)

Tests:

- **AC-1 (no crash on blank id):** `load_workflow_dir(slug_fixture_root, slug_fixture_root)` returns `Ok` and the snapshot's `items` contains the entity with **no** entry in `parse_errors`. Asserts the blank-id entity parses instead of becoming a broken row.
- **AC-2 / slug-identity path (the required slug-identity test):** the parsed `WorkItem.id == "roadmap-v5"` (filename stem), proving the slug is used as the effective ID that the overview ID column renders. Pairs with a thin `src/ui/list.rs` test (or assertion on the rendered `format!("{:>4}", item.id)` value) confirming the slug appears in the ID column string — kept as a pure-string check so it needs no terminal.
- **AC-3 (id-style read + both values exercised):** one test parses the slug README and asserts `definition.id_style.as_deref() == Some("slug")`; a second uses an existing/`write_markdown` README with `id-style: sequential` and asserts `Some("sequential")`. Exercises both branch values per the AC.
- **AC-4 (sequential unaffected):** a temp `sequential` workflow whose entity has a populated numeric `id` still parses to that id; and a `sequential` (or default) workflow with a *blank* id still errors with `MissingRequiredField{ field: "id" }` — locking that the tolerance is slug-scoped. The full existing suite must also stay green.

### Verification commands

- AC-1 + AC-2: `cargo test --lib parser::tests` (covers the slug-fixture load + slug-id assertions). The specific proving invocation for AC-1 and AC-2 is:
  `cargo test --lib -- parser::tests::loads_slug_workflow_uses_filename_as_id`
  (name the new test exactly so the validation stage can run it by name; add the AC-1 no-crash assertion in the same test or a sibling `slug_workflow_blank_id_does_not_error`).
- Full regression / AC-4: `cargo test` (all unit + integration), then `make lint` (clippy `-D warnings`, per CLAUDE.md lint gate) before marking implementation complete.

### Module / ownership notes for worktree execution

- Single-crate, single-worktree task. All edits are under `src/parser/` (item.rs, snapshot.rs) and new fixtures under `tests/fixtures/slug-workflow/`. No `agents/` or `references/` scaffolding touched; no frontmatter edits to existing entities.
- The slug-identity logic lands entirely in the parser/domain layer (`parse_work_item`), so it is unit-testable with `load_workflow_dir` and `parse_work_item` — no ratatui backend required. This satisfies the checklist's separation of parser/state work from TUI rendering.
- Read-only invariant (CLAUDE.md): no new git/filesystem mutation of workflow files; fixtures are test-owned files under `tests/`.

## Stage Report: plan

- DONE: Plan names specific Rust files to change and the `cargo test` invocation that proves AC-1 and AC-2
  Files: `src/parser/item.rs` (id resolution + new `id_style` param), `src/parser/snapshot.rs` (caller passes `definition.id_style`); proof: `cargo test --lib -- parser::tests::loads_slug_workflow_uses_filename_as_id` plus `cargo test --lib parser::tests`.
- DONE: Test strategy identifies at least one new fixture file and one test covering the slug-identity path
  New fixture dir `tests/fixtures/slug-workflow/` (README + `roadmap-v5.md` with blank `id:`); slug-identity test asserts `WorkItem.id == "roadmap-v5"` from `file_stem()`.
- DONE: Parser/state changes are separated from TUI rendering changes so the slug-identity logic is testable without a terminal
  All slug resolution lands in `parse_work_item`/`snapshot.rs` (parser layer); `src/ui/list.rs:136` already renders `item.id` unchanged, so AC-2 is provable via `load_workflow_dir` with no ratatui backend.

### Summary

Wrote an implementation plan grounded in the current code: the only crash point is `required(raw.id, …)` at `src/parser/item.rs:29`, and its sole caller `snapshot.rs:23` already holds `definition.id_style`, so threading `id_style` into `parse_work_item` keeps slug resolution in the parser/domain layer and fully testable without a TUI. `WorkflowDefinition.id_style` is already parsed (`readme.rs:69`) and even displayed (`definition.rs:90`), so AC-3 reduces to adding a fixture exercising both `sequential` and `slug`. Effective slug = filename stem (matching the reference workflow); blank-id tolerance is scoped to slug workflows only so AC-4 sequential behavior is preserved. Noted one follow-up: the `{:>4}` ID column in `list.rs` visually right-truncates wide slugs — flagged for the validation stage, not fixed in this slice.

## Stage Report: implement

- DONE: `cargo test` passes — including new slug-identity tests proving AC-1 (no crash on blank id) and AC-2 (WorkItem.id equals filename stem)
  Full suite 340 lib + integration tests pass (0 failed); new `parser::tests::loads_slug_workflow_uses_filename_as_id` asserts `WorkItem.id == "roadmap-v5"` and no `parse_errors`; `slug_workflow_blank_id_does_not_error` is the explicit AC-1 no-crash guard.
- DONE: `make lint` passes — clippy -D warnings clean per CLAUDE.md lint gate
  `make lint` finished with no warnings after the changes.
- DONE: Blank-id tolerance is scoped to id-style: slug only — a sequential workflow with blank id still errors with MissingRequiredField (AC-4 preserved)
  `resolve_id` (`src/parser/item.rs`) only derives the slug when `id_style == Some("slug")`; `sequential_workflow_id_behavior_unaffected` proves blank id under both default and explicit `sequential` still yields `MissingRequiredField{ field: "id" }`, and a populated numeric id round-trips.

### Summary

Threaded `id_style: Option<&str>` through `parse_work_item` and its three production callers (`snapshot.rs`, `worktree.rs`, `archive.rs` + the `overview.rs` archive caller), passing `definition.id_style.as_deref()`. The single crash point `required(raw.id, …)` became a new `resolve_id` helper: a populated `id:` always wins; a blank id resolves to the filename stem only when `id-style: slug`; any other blank id keeps today's `MissingRequiredField` error. Slug derivation mirrors `worktree::slug_of_path` (parent dir for `index.md` folder-form, file stem otherwise). Added committed fixture `tests/fixtures/slug-workflow/` (README with `id-style: slug` + blank-id `roadmap-v5.md`) and four parser tests covering AC-1–AC-4. AC-3's `id_style` was already parsed in `readme.rs`, so no parser change was needed there — only a test exercising both `slug` and `sequential`. Follow-up still open (from plan): `src/ui/list.rs` ID column is `{:>4}`, which visually right-pads/truncates slugs wider than 4 chars (e.g. `roadmap-v5` overflows the field but is not dropped); widening that column is left for the validation stage.

## Stage Report: review

- DONE: Verdict is explicit (PASSED or REJECTED) with per-AC evidence citations — not just "tests passed"
  **VERDICT: PASSED.** AC-1 (no crash on blank id): `slug_workflow_blank_id_does_not_error` + `loads_slug_workflow_uses_filename_as_id` load the committed slug fixture via `load_workflow_dir` and assert `Ok` with empty `parse_errors` — both run green. AC-2 (slug as display id): `loads_slug_workflow_uses_filename_as_id` asserts `WorkItem.id == "roadmap-v5"` (filename stem) and that `format!("{:>4}", item.id)` — the literal `src/ui/list.rs:136` ID-column expression — contains the slug; verified `list.rs:136` is unchanged and renders `item.id` directly. AC-3 (id-style from README, both values): `id_style_read_from_readme_for_both_values` asserts `Some("slug")` from the committed fixture and `Some("sequential")` from a temp README. AC-4 (sequential unaffected): `sequential_workflow_id_behavior_unaffected` proves a populated numeric id round-trips and a blank id under both default and explicit `sequential` still errors `MissingRequiredField{ field: "id" }`; full suite 340 lib + integration green, 0 failed, `make lint` clean.
- DONE: The plan's flagged follow-up ({:>4} ID column width for wide slugs) is ruled in or out of scope with reasoning
  **OUT OF SCOPE — cosmetic only.** `format!("{:>4}", item.id)` is Rust minimum-width right-alignment: it pads short ids to 4 chars and never truncates wider ones. The id is emitted as a free `Span` in a `Line` (`list.rs:160-168`), so ratatui clips only at the terminal's right edge, not at a 4-char cell. A 10-char slug therefore shifts the worktree-marker/title columns rightward on that row (misalignment), with no id truncation and no data loss. The plan's "right-truncates" wording is imprecise but the user-visible effect is harmless. None of the four ACs require fixed-width id alignment, so widening the column is a genuine follow-up, correctly deferred.
- DONE: Review challenges whether slug-identity fallback is correct for all three callers (snapshot.rs, worktree.rs, archive.rs) — not just the happy path
  Identity style is a workflow-level property, so all three callers must resolve identity from the *same* README. Verified each sources `id_style` from the one parsed `WorkflowDefinition`: `snapshot.rs:19,23,34` derives `definition.id_style.as_deref()` and threads it to both active-item parsing and `scan_worktrees`; `worktree.rs:49,68` only forwards that same value to `parse_work_item` (no independent README read); `archive.rs` receives it from `overview.rs:690` (`self.snapshot.definition.id_style.as_deref()` — same definition). Worktree and archived entities of a slug workflow are the same entities in a different lifecycle location, so resolving them with the workflow's `id_style` is correct, not just the active happy path. No caller re-reads or diverges; a non-slug workflow's blank id still errors uniformly across all three paths.

### Summary

PASSED. The change is tightly scoped: one `resolve_id` helper in `src/parser/item.rs` plus a mechanical `id_style: Option<&str>` thread through `parse_work_item` and its three production callers, all sourcing the same workflow-level `id_style` — verified correct for the worktree and archive paths, not just active items. All four ACs are backed by named parser tests that exercise behavior (slug fixture load, `WorkItem.id` value, both id-style values, sequential error preservation); full suite 340+ tests green and clippy `-D warnings` clean. The `{:>4}` ID-column follow-up is correctly out of scope: confirmed it is cosmetic (no truncation, no crash, no dropped data) and unrelated to any AC. Ready to move to done.
