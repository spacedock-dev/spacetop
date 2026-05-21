---
id: "043"
title: Stage-heading parser drops prose when the README uses `### `stage` (qualifier)` form
status: review
source: captain
started: 2026-05-21T07:55:59Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-043-stage-heading-with-qualifier-not-parsed
issue:
pr: #41
mod-block: merge:pr-merge
---

When the captain opens `spacetop -w /Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/` and presses `D` to show the workflow README detail, the per-stage description blocks are all empty even though the README clearly defines them under a `## Stages` section.

## Reproduction

```bash
spacetop -w /Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/
# press D to open the workflow README detail view
# every stage row shows an empty description
```

The README at that path uses stage headings of the form:

```
### `pending`
### `scoping` (lead only, worktree)
### `ideate` (question only, no worktree)
### `review` (hypothesis only, gate, fresh)
### `smoke` (hypothesis only, worktree)
### `run` (hypothesis only, worktree)
### `analyze` (hypothesis only, fresh, no worktree)
### `promote` (hypothesis only, gate, fresh)
### `expanded` / `ideated` / `done` / `rejected`
```

The only headings whose prose actually populates the per-stage map are `### \`pending\`` and (effectively) the combined `### \`expanded\` / \`ideated\` / \`done\` / \`rejected\`` line — and the latter populates the wrong key. Every stage that has a parenthetical qualifier after the backticked name (`scoping`, `ideate`, `review`, `smoke`, `run`, `analyze`, `promote`) loses its prose entirely.

## Diagnosis

`src/parser/readme.rs::stage_heading_name` (line 130) strips the `### ` prefix, then runs `trim_matches('\`')` on the remainder. That only strips backticks from the **outer** edges. For a heading like `\`scoping\` (lead only, worktree)`:

- after `strip_prefix("### ")` → `\`scoping\` (lead only, worktree)`
- after `trim()` → unchanged
- after `trim_matches('\`')` → `scoping\` (lead only, worktree)` — only the leading backtick is removed because the trailing char is `)`, not a backtick

The resulting string `scoping\` (lead only, worktree)` is stored as the key in the prose `HashMap<String, String>`. Later when the renderer looks up `definition.stage_prose.get("scoping")` to render the detail view, it finds nothing — the key on file is the full noisy string. So the rendered detail pane shows the stage name from frontmatter but no description body.

The README contract for spacetop is implicit and lenient elsewhere (frontmatter accepts varied shapes, the YAML parser is permissive), so the heading parser's strictness about "name must be the entire trimmed-and-unticked content" is the outlier. Other Spacedock workflows in real use (notably the bigger research/hypothesis-flavoured workflows) routinely add a parenthetical qualifier on stage headings as documentation, and the parser should accept that.

## Acceptance criteria

**AC-1 — Stage prose populates for the canonical research-workflow heading form.**
Verified by: a unit test in `src/parser/readme.rs::tests` that feeds the parser a `## Stages` block with headings of the form `### \`scoping\` (lead only, worktree)`, `### \`review\` (hypothesis only, gate, fresh)`, and a few siblings — all matching the frontmatter `stages.states` names — and asserts the returned `HashMap<String, String>` has an entry for each plain stage name (`"scoping"`, `"review"`, …) whose value is the prose body that followed that heading.

**AC-2 — Stage prose still populates for the simple `### \`stage\`` form.**
Verified by: the existing `parse_workflow_readme_populates_stage_prose` test (or its equivalent) continues to pass unchanged. The fix must be additive — accepting the qualifier-suffixed form without breaking the strict form.

**AC-3 — Slash-joined heading like `### \`expanded\` / \`ideated\` / \`done\` / \`rejected\`` is recognized for every named stage in the list.**
Verified by: a unit test that asserts each of the four names becomes a key in the returned map, and they all share the same prose body (the prose under that combined heading applies to all four terminal stages). If the design stage decides this case is out of scope for the fix, the test instead asserts that at least one of the four names is keyed (today: none are) and the entity body's design notes explicitly defer the multi-stage form to a follow-up task.

**AC-4 — The TUI detail view (D-key) renders the matched prose under each stage when running against the research workflow.**
Verified by: an integration test or a rendering test in `src/ui/` that loads the research workflow fixture (a trimmed copy in `tests/fixtures/` is fine), opens the workflow detail view, and asserts the rendered buffer contains a recognisable substring from each stage's prose body (e.g. for `scoping`, a phrase that appears in the real README's `scoping` paragraph). At a minimum, assert non-empty rendered prose for `scoping`, `review`, `smoke`, `analyze`, and `promote`.

**AC-5 — `make lint` and `cargo test` pass.**
Verified by: `make lint` (clippy `-D warnings`) and `cargo test` from the repo root, both green; pre-existing parser tests untouched or updated in lockstep with the new heading-shape coverage.

## Suggested fix sketch (for the design / implement stages)

The parser needs to extract the first backtick-wrapped token from the heading and treat that as the stage name, ignoring any trailing prose. Pseudocode:

```rust
fn stage_heading_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("### ")?;
    let trimmed = rest.trim();
    // Prefer backtick-delimited token: `name`
    if let Some(name) = first_backtick_token(trimmed) {
        return Some(name);
    }
    // Fallback: whole trimmed line, only if it has no inline backticks at all
    if !trimmed.contains('`') {
        return Some(trimmed);
    }
    None
}
```

For AC-3, the parser may need to scan for *all* backtick tokens on the heading line and emit the same prose body under each one — the design stage should weigh in on whether to handle the multi-stage combined heading or punt.

## Plan

### Scope decision: AC-3 in scope, single-pass parser change

We will handle the slash-joined heading inside this task. Reasoning:

- The fix shape is identical to the AC-1 fix (scan backtick tokens on the heading line); supporting one is supporting all.
- Deferring AC-3 leaves the research workflow with four broken terminal stages (`expanded`, `ideated`, `done`, `rejected`) and doubles the test fixtures we have to maintain across two cycles.
- The implementation cost is one extra collected `Vec<String>` plus a loop that inserts the same body under each name when closing a block — strictly smaller than the test scaffolding cost of punting.

### Parser change (src/parser/readme.rs)

Two surgical edits to the file, no new module:

1. **Replace `stage_heading_name(&str) -> Option<&str>`** (line 130) with `stage_heading_names(&str) -> Option<Vec<&str>>`. The new function:
   - Strips the `### ` prefix and rejects a second `#` (defence in depth, unchanged).
   - Walks the trimmed rest, collecting every backtick-wrapped token: scan for an opening `` ` ``, then the next `` ` ``, push the slice between them, advance past the closing tick, repeat. Anything outside the tick pairs is ignored (so qualifiers like ` (lead only, worktree)` and slash separators ` / ` are skipped).
   - If at least one token was found, return `Some(tokens)`.
   - Otherwise (no backticks at all on the line), fall back to the entire trimmed rest as a single-element vec — this preserves AC-2 for the legacy plain-text form `### plan` if anyone uses it.
   - Returns `None` when the trimmed rest is empty.
   - Helper kept inline in the same module; no new pub surface.

2. **Update `parse_stage_prose`'s loop** (around the three `current.take()` sites at lines 96–124). Change `current` from `Option<(String, String)>` to `Option<(Vec<String>, String)>`. When a stage block closes, iterate the names vector and insert the same prose body under each name into `out`. The block-collection logic (pushing lines into `prose`, trimming trailing blanks) is unchanged. The lifetime story stays simple because we eagerly `.to_string()` each name when we open the block.

The fallback rule (no backticks at all → whole trimmed line is the name) is deliberately narrow: it preserves AC-2 and avoids ambiguity. Any line with backticks must use them to delimit names — partial-tick weirdness like `` `foo bar `` (unterminated) is treated as "no tokens found" and the heading is dropped silently, matching today's behaviour for malformed input.

No name normalisation, no whitespace collapsing inside a token, no case-folding. The scan extracts the byte slice between two `` ` `` characters verbatim and stores it as-is. This mirrors how the existing parser treats stage names.

### Acceptance-criteria test strategy

All parser-level tests live in `src/parser/readme.rs::tests` (the existing `#[cfg(test)] mod tests` at the bottom of the file). Inline string-literal fixtures only — no new files under `tests/fixtures/`. Rationale: the existing tests in this module already use inline READMEs for similar coverage (`prose_extracts_stage_body_verbatim`, `prose_missing_block_is_silent`), and a fixture file would add filesystem coupling for no readability win.

- **AC-1 — qualifier-suffixed headings populate prose.** New test `prose_extracts_qualifier_suffixed_headings`. Inline README with `## Stages` containing `` ### `scoping` (lead only, worktree) ``, `` ### `review` (hypothesis only, gate, fresh) ``, and `` ### `smoke` (hypothesis only, worktree) ``, each with a one-line body. Asserts:
  - `out.get("scoping") == Some("scoping-body")`
  - `out.get("review") == Some("review-body")`
  - `out.get("smoke") == Some("smoke-body")`
  - The noisy form `scoping\` (lead only, worktree)` is NOT a key (negative assertion to lock the trim contract).

- **AC-2 — plain `### \`stage\`` form still works.** Covered by the existing `prose_extracts_stage_body_verbatim` test and `prose_extracts_real_readme_plan_stage` — both must continue to pass unchanged. No edit required; the implement worker just runs them. The fix is additive by construction.

- **AC-3 — slash-joined heading populates every named stage.** New test `prose_extracts_slash_joined_terminal_stages`. Inline README with `` ### `expanded` / `ideated` / `done` / `rejected` `` followed by a one-line body `terminal-shared-body`. Asserts:
  - For each of the four names, `out.get(name) == Some("terminal-shared-body")`.
  - All four entries share byte-identical bodies.

- **AC-4 — TUI detail view renders matched prose.** The D-key handler and Definition-mode renderer already exist (`src/ui/definition.rs`, see `definition_renders_against_real_readme` test at line 476, which uses `App::load` + `handle_key(KeyCode::Char('D'))` + `TestBackend` + `buffer_text`). Add a new test in the same `src/ui/definition.rs::tests` module: `definition_renders_qualifier_suffixed_stages`. It builds a temp workflow directory with a `README.md` whose frontmatter declares stages `[scoping, review, smoke, analyze, promote]` and a `## Stages` section using the qualifier-suffixed heading form for each, with a distinctive one-line body per stage (e.g. `"scoping-prose-marker"`). Then it calls `App::load(temp_root)`, dispatches `KeyCode::Char('D')`, renders via `crate::ui::render` against a `TestBackend::new(160, 80)`, and asserts the buffer contains each `<stage>-prose-marker` substring. This exercises the full D-key path end to end, mirroring the existing `definition_renders_against_real_readme` shape so the implement worker has a one-to-one template.

  Why a synthetic temp workflow instead of trimming the real dataagentbench README into `tests/fixtures/`: the existing AC-4-style test in this module (`definition_renders_against_real_readme`) already proves the docs/spacetop-dev README path. A second test against a synthetic README with the qualifier-suffixed form keeps the failure mode focused — when this test breaks, the implement worker knows the qualifier-handling regressed, not some incidental dataagentbench README detail. The TempDir + write-README pattern is already used several times in the same test module (see `definition_header_carries_active_tab_basename` around line 511) so no new test infrastructure is needed.

- **AC-5 — lint + tests green.** Final gate: `make lint` (clippy `-D warnings`) and `cargo test` from `/Users/kent/Dev/InfuseAI/GitHub/spacetop`. No new warnings expected: the parser change replaces one helper with another of the same arity, and the loop changes are mechanical (`String` → `Vec<String>` collection + iter at close time).

### Files touched

- `src/parser/readme.rs` — replace `stage_heading_name` with `stage_heading_names`; update `parse_stage_prose` loop. Add two new tests in the same module's `tests` submodule.
- `src/ui/definition.rs` — add one new test `definition_renders_qualifier_suffixed_stages` in the existing `tests` submodule. No production code changes here; the renderer already does the right thing once the parser returns the correct map.

No changes to `src/domain/`, `src/app.rs`, `src/ui/mod.rs`, or any other file. The fix is contained to the parser plus its UI-level integration test.

### Verification commands (for implement / review)

From the repo root `/Users/kent/Dev/InfuseAI/GitHub/spacetop`:

- `cargo test parser::readme::tests` — runs the four parser-prose tests (existing two + two new) in isolation; quickest signal.
- `cargo test ui::definition::tests::definition_renders_qualifier_suffixed_stages` — the AC-4 rendering test.
- `cargo test` — full test suite, including the two prior real-README tests (`prose_extracts_real_readme_plan_stage`, `parse_workflow_readme_populates_stage_prose`, `definition_renders_against_real_readme`) which must still pass unchanged.
- `make lint` — clippy gate (`-D warnings`); required by CLAUDE.md before completion.

No worktree is required for the implement stage; the parser/UI changes are contained and the entity does not declare `worktree: true` in the spec. The implement worker should still confirm by checking the entity frontmatter and the workflow README's stage declarations at dispatch time.

## Stage Report: plan

- DONE: Parser change: name the exact edit to `stage_heading_name` in `src/parser/readme.rs:130` — the new extraction rule (first backtick-wrapped token vs. fallback whole-trimmed-line) and how it interacts with the existing `parse_stage_prose` loop. Identify any helper to add (e.g. `first_backtick_token`) and confirm whether multiple backtick tokens on one line should each receive the same prose body (AC-3 scope decision).
  Plan section "Parser change" specifies replacing `stage_heading_name` with `stage_heading_names(&str) -> Option<Vec<&str>>`, the byte-slice scan loop for backtick pairs, the no-backtick fallback that preserves AC-2, and the `current: Option<(Vec<String>, String)>` change in `parse_stage_prose` so each name in the vec receives the same prose body.
- DONE: Decide AC-3 scope: handle the slash-joined `### \`expanded\` / \`ideated\` / \`done\` / \`rejected\`` heading by emitting the prose under each named stage, OR defer to a follow-up task with a recorded rationale. Name the test assertions that lock the chosen behavior. If deferring, the design notes in the entity body must say so explicitly and AC-3's softer form (assert at least one key populated) must be cited.
  In scope — rationale recorded under "Scope decision". Test `prose_extracts_slash_joined_terminal_stages` asserts all four names map to the same body.
- DONE: Test strategy for AC-1..AC-5: name the test module locations (likely `src/parser/readme.rs::tests` and a new UI test or integration test for AC-4), the fixture shape (inline string literal vs. file under `tests/fixtures/`), and the assertion APIs (HashMap key+value equality for parser tests; TestBackend buffer substring for the AC-4 D-key rendering test). For AC-4 in particular, identify the existing UI test infrastructure used to drive the workflow-readme detail view (the D-key handler) — name the handler/state code paths so the implement worker knows where to plug in.
  "Acceptance-criteria test strategy" lists each AC's module/test name, the inline-literal fixture choice, and pins AC-4 to `src/ui/definition.rs::tests::definition_renders_qualifier_suffixed_stages`, modelled on the existing `definition_renders_against_real_readme` (line 476) which already exercises `App::load` + `KeyCode::Char('D')` + `TestBackend` + `buffer_text`.

### Summary

Plan commits to handling both the qualifier-suffixed and slash-joined heading forms in a single parser change: replace `stage_heading_name` with a `stage_heading_names` byte-scan that collects every backtick-wrapped token on the heading line, and adapt `parse_stage_prose` to insert the same body under each name. Tests live entirely in-tree (`src/parser/readme.rs::tests` for AC-1/AC-3, `src/ui/definition.rs::tests` for AC-4) using inline string literals and the existing `TestBackend` + `App::load` infrastructure that the current D-key test already uses — no new fixture files, no new test infrastructure. AC-5 is the standard `make lint` + `cargo test` gate.

## Stage Report: implement

- DONE: Parser fix per plan: replace `trim_matches('`')` in `stage_heading_name` (src/parser/readme.rs:130) with first-backtick-token extraction (and the AC-3 scope decision the plan made — slash-joined multi-stage heading either handled by emitting prose under each named stage, or explicitly deferred). Cite the new helper function and how it interacts with `parse_stage_prose`'s loop.
  Replaced `stage_heading_name` with `stage_heading_names(&str) -> Option<Vec<&str>>` that byte-scans for every backtick-delimited token (falls back to whole trimmed line when no backticks). `parse_stage_prose`'s `current` is now `Option<(Vec<String>, String)>` and inserts the same trimmed prose body under each name when a block closes — AC-3 handled in scope per plan.
- DONE: Tests covering AC-1..AC-5 per the plan's test strategy: qualifier-suffixed headings populate the prose map by plain stage name (AC-1); simple `### \`stage\`` form still works (AC-2); slash-joined multi-stage heading per the plan's AC-3 decision; TUI D-key detail view renders non-empty prose for `scoping`, `review`, `smoke`, `analyze`, `promote` against the research fixture (AC-4); `make lint` clean and full `cargo test` green (AC-5/AC-6).
  Added `prose_extracts_qualifier_suffixed_headings` and `prose_extracts_slash_joined_terminal_stages` in `src/parser/readme.rs::tests`, plus `definition_renders_qualifier_suffixed_stages` in `src/ui/definition.rs::tests`. Full suite: 291 lib + 4 + 10 integration = 305/305 passed; `make lint` clean (clippy -D warnings).
- DONE: Smoke verification: from the worktree, build release and run `spacetop -w /Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/` long enough to confirm the loader returns 12 stages with populated `stage_prose` entries.
  Ran a temporary `examples/smoke_043.rs` calling `parse_workflow_readme` on `/Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/README.md`: 12 stages, all 12 (`pending`, `scoping`, `ideate`, `review`, `smoke`, `run`, `analyze`, `promote`, `expanded`, `ideated`, `done`, `rejected`) report POPULATED. Smoke file deleted before commit.

### Summary

Fix is localised to `src/parser/readme.rs` exactly as planned: `stage_heading_names` now collects every backtick-wrapped token from the heading line and `parse_stage_prose` emits the same prose body under each name, covering both qualifier-suffixed (`### \`scoping\` (lead only, worktree)`) and slash-joined (`### \`expanded\` / \`ideated\` / \`done\` / \`rejected\``) forms. Three new tests pin the two parser shapes plus the TUI D-key rendering against a synthetic five-stage fixture. End-to-end smoke against the real dataagentbench research README confirms all 12 stages now have populated prose.
