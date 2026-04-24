---
id: 002
title: Parse Spacedock Workflow Files
status: done
source: commission seed
started: 2026-04-24T14:30:53Z
completed: 2026-04-24T15:15:30Z
verdict: PASSED
score: 1.0
worktree: 
issue:
pr:
mod-block: 
archived: 2026-04-24T15:15:30Z
---

Read Spacedock workflow `README.md` metadata and work item markdown frontmatter into a typed model that SpaceTop can use for status summaries and workflow structure views.

## Acceptance criteria

**AC-1 -- Workflow README frontmatter is parsed into typed stage metadata.**
Verified by: parser tests using `docs/spacetop-dev/README.md` or fixture copies assert stage names, initial state, terminal state, and review gate metadata.

**AC-2 -- Work item markdown frontmatter is parsed into typed task records.**
Verified by: parser tests assert IDs, titles, statuses, sources, scores, and body text for the seed task files.

**AC-3 -- Invalid or incomplete workflow files produce actionable errors.**
Verified by: tests cover missing frontmatter, unknown status, and malformed YAML with readable error messages.

## Implementation plan

This task should land after `001 Scaffold Rust CLI Project` creates the crate and baseline module layout. Keep the feature read-only: all parsing APIs should take paths or strings and return typed data without writing to workflow files.

1. Add parser-oriented dependencies if the scaffold has not already done so:
   - `serde` with `derive`
   - `serde_yaml`
   - `thiserror` for readable parser errors
   - optionally `camino` only if the scaffold already prefers UTF-8 path handling
2. Create a domain module for workflow data, expected as `src/domain.rs` or `src/domain/mod.rs` depending on the scaffold:
   - `WorkflowDefinition { root, stages, id_style, entity_type, labels }`
   - `StageDefinition { name, initial, terminal, gate, fresh, feedback_to, worktree, concurrency }`
   - `WorkItem { path, id, title, status, source, started, completed, verdict, score, worktree, issue, pr, body }`
   - `WorkflowSnapshot { definition, items }`
3. Create parser code under `src/parser.rs` or `src/parser/mod.rs`:
   - `parse_workflow_readme(path: &Path) -> Result<WorkflowDefinition, ParseError>`
   - `parse_work_item(path: &Path, allowed_statuses: &[String]) -> Result<WorkItem, ParseError>`
   - `load_workflow_dir(path: &Path) -> Result<WorkflowSnapshot, ParseError>`
4. Implement frontmatter extraction as a small shared helper that recognizes a leading YAML block delimited by `---`, returns `(frontmatter, body)`, and reports missing or unterminated frontmatter with the file path.
5. Deserialize README frontmatter into serde structs that mirror `docs/spacetop-dev/README.md`, then normalize stage defaults and per-stage overrides into the typed `StageDefinition` list.
6. Deserialize work item frontmatter into a typed raw struct, preserve the markdown body after frontmatter, and validate `status` against the README-derived stage names.
7. Make invalid files actionable:
   - include file path and field/context in each `ParseError`
   - report missing required task fields (`id`, `title`, `status`)
   - report malformed YAML distinctly from unknown status
8. Wire parser loading into the CLI/app boundary only enough for the binary to call it later; keep terminal rendering out of this task.
9. Run verification:
   - `cargo fmt --check`
   - `cargo test`
   - optional manual smoke command once the CLI accepts a workflow path, e.g. `cargo run -- --workflow-dir docs/spacetop-dev`

## Focused test strategy

Parser tests should live beside the parser implementation as unit tests, with small inline fixtures for error cases and one integration-style fixture path using `docs/spacetop-dev`.

- README metadata test: parse `docs/spacetop-dev/README.md` and assert stages `design`, `plan`, `implement`, `review`, `done`; assert `design.initial == true`, `done.terminal == true`, `review.gate == true`, `review.feedback_to == Some("implement")`, and `implement.worktree == true`.
- Work item metadata test: parse the seed task files under `docs/spacetop-dev/*.md` except `README.md`; assert `id`, `title`, `status`, `source`, `score`, and non-frontmatter markdown body are preserved.
- Directory loader test: load `docs/spacetop-dev` and assert the snapshot contains one workflow definition and the expected task count from non-README markdown files, ignoring `_mods`.
- Missing frontmatter test: pass a temp or inline markdown string without leading `---` and assert the error names missing frontmatter.
- Unknown status test: parse a work item fixture with `status: impossible` against the README stage set and assert the error includes the unknown status and allowed context.
- Malformed YAML test: parse a fixture with invalid YAML and assert the error is distinct from validation failures.

## File and module ownership for implementation

Expected implementation files for the later worktree stage:

- `Cargo.toml`: parser dependencies only if they are not already present from the scaffold.
- `src/domain.rs` or `src/domain/mod.rs`: typed workflow and work item model.
- `src/parser.rs` or `src/parser/mod.rs`: frontmatter extraction, serde deserialization, directory loading, and parser tests.
- `src/lib.rs`: export domain/parser modules if the scaffold creates a library target.
- `src/main.rs` or CLI module: only minimal wiring needed to prove parser load works; no TUI rendering changes.

Ownership boundary: the implementation worker should not modify `docs/spacetop-dev/*.md` workflow state files except through its own stage report if dispatched for this entity. The TUI overview task owns rendering behavior and should consume `WorkflowSnapshot` rather than duplicating markdown parsing.

## Stage Report: plan

- DONE: DONE/SKIPPED/FAILED accounting must show a concrete implementation plan for parsing workflow README frontmatter and work item markdown frontmatter.
  Added `Implementation plan` with parser APIs, domain types, frontmatter extraction, README stage normalization, work item validation, and verification commands.
- DONE: DONE/SKIPPED/FAILED accounting must show a focused parser test strategy with representative fixtures or files.
  Added `Focused test strategy` covering `docs/spacetop-dev/README.md`, seed task files, directory loading, missing frontmatter, unknown status, and malformed YAML.
- DONE: DONE/SKIPPED/FAILED accounting must identify file/module ownership for the later implementation stage.
  Added `File and module ownership for implementation` naming expected Rust files and separating parser ownership from TUI rendering.

### Summary

Planned the parser work as a read-only Rust domain/parser layer that can be tested without a terminal backend. The plan accounts for the current repository state, where the Rust crate has not been scaffolded yet, and keeps later TUI work dependent on a typed `WorkflowSnapshot` rather than markdown parsing in rendering code.

## Stage Report: implement

- DONE: DONE/SKIPPED/FAILED accounting must show workflow README frontmatter is parsed into typed stage metadata with defaults/overrides.
  Added `parse_workflow_readme` and `WorkflowDefinition`/`StageDefinition`; tests assert `docs/spacetop-dev/README.md` stages, initial/terminal flags, review gate metadata, and default concurrency.
- DONE: DONE/SKIPPED/FAILED accounting must show work item frontmatter and markdown body are parsed into typed task records.
  Added `parse_work_item` and `WorkItem`; tests assert task id/title/status/source/score/worktree fields and body preservation.
- DONE: DONE/SKIPPED/FAILED accounting must show invalid/malformed files produce actionable errors and `cargo fmt --check` / `cargo test` evidence.
  Tests cover missing frontmatter, malformed YAML, missing required fields, and unknown status; `cargo fmt --check` passed and `cargo test` passed with 11 tests.

### Summary

Implemented read-only Spacedock workflow parsing behind `src/parser.rs` with typed domain models in `src/domain/mod.rs`. The directory loader builds a `WorkflowSnapshot` from top-level workflow markdown while ignoring README and subdirectory workflow support files; no TUI rendering or workflow-state writes were added.

## Stage Report: review

- DONE: DONE/SKIPPED/FAILED accounting must show whether AC-1 (workflow README stage metadata parsing) is satisfied with fresh evidence.
  `src/parser.rs` defines `parse_workflow_readme`; fresh `cargo test` showed `parser::tests::parses_workflow_readme_stage_metadata_with_defaults_and_overrides ... ok`.
- FAILED: DONE/SKIPPED/FAILED accounting must show whether AC-2 (work item frontmatter/body parsing) is satisfied with fresh evidence.
  Fresh `cargo test` failed because parser tests still expect this task's mutable workflow status to be `implement`, but the review transition commit changed it to `review`.
- FAILED: DONE/SKIPPED/FAILED accounting must show whether AC-3 (actionable parser errors) is satisfied with fresh evidence, plus `cargo fmt --check` and `cargo test` results.
  Error-path tests passed and `cargo fmt --check` passed, but fresh `cargo test` failed overall: 9 passed, 2 failed (`parses_work_item_frontmatter_and_preserves_markdown_body`, `loads_workflow_snapshot_from_directory_ignoring_mods_and_archive`).

### Summary

Verdict: REJECTED. The parser implementation appears scoped and includes actionable error variants, but the branch cannot be approved while the claimed `cargo test` evidence no longer reproduces; the failing tests are coupled to workflow state that changed when this entity entered review.

## Stage Report: implement

- DONE: DONE/SKIPPED/FAILED accounting must show the tests no longer depend on the live mutable status of this workflow task.
  Replaced the exact work item status test with a temp markdown fixture and changed the live directory assertion to require task `002` status membership in README-defined stages.
- DONE: DONE/SKIPPED/FAILED accounting must show the original parser acceptance criteria remain covered.
  README parsing, stable work item frontmatter/body parsing, directory loading, and parser error tests remain covered in `src/parser.rs`.
- DONE: DONE/SKIPPED/FAILED accounting must show fresh `cargo fmt --check` and `cargo test` evidence after the fix.
  `cargo fmt --check` passed; `cargo test` passed with 11 tests.

### Summary

Fixed the review rejection by removing test coupling to the mutable status of `docs/spacetop-dev/parse-spacedock-workflow-files.md`. The exact work item metadata/body assertions now use a stable temp fixture, while the live workflow directory test verifies item loading and allowed status validation without hard-coding the current stage.

## Stage Report: review (cycle 2)

- DONE: DONE/SKIPPED/FAILED accounting must show whether AC-1 (workflow README stage metadata parsing) is satisfied with fresh evidence.
  `parse_workflow_readme` maps README stage defaults/overrides into typed stages; fresh `cargo test` passed `parser::tests::parses_workflow_readme_stage_metadata_with_defaults_and_overrides`.
- DONE: DONE/SKIPPED/FAILED accounting must show whether AC-2 (work item frontmatter/body parsing) is satisfied with fresh evidence, including the mutable-status regression fix.
  `parse_work_item` parses typed frontmatter and body from a stable temp fixture, while `load_workflow_dir` now asserts live task `002` status is any README-defined stage instead of hard-coding `implement`; fresh `cargo test` passed all 11 tests.
- DONE: DONE/SKIPPED/FAILED accounting must show whether AC-3 (actionable parser errors) is satisfied with fresh evidence, plus `cargo fmt --check` and `cargo test` results.
  Error tests cover missing frontmatter, malformed YAML, missing required field, and unknown status messages; `cargo fmt --check` passed and `cargo test` passed with 11 tests.

### Summary

Verdict: PASSED. The feedback fix resolves the rejected-review regression without weakening the parser acceptance coverage, and the implementation remains read-only and scoped to typed workflow README/work item parsing with actionable error messages.
