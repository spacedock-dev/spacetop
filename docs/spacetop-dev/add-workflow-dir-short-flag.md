---
id: 004
title: Add short `-w` alias for `--workflow-dir`
status: implement
source: captain feedback after build-initial-tui-overview
started: 2026-04-24T16:04:53Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-add-workflow-dir-short-flag
issue:
pr:
mod-block: merge:pr-merge
---

Expose `-w` as a short alias for the existing `--workflow-dir` CLI flag so the common invocation (`spacetop -w docs/spacetop-dev`) is ergonomic. Scope is strictly the CLI surface; no behavior change.

## Problem statement and user flow

Today the only way to point spacetop at a workflow directory is the long flag `--workflow-dir <PATH>`. Captains inspecting Spacedock workflows routinely re-invoke the binary against different paths during development, and the 15-character long form adds friction to that tight loop. The short alias `-w` is the conventional, discoverable ergonomic fix: `spacetop -w docs/spacetop-dev` reads naturally and matches common CLI practice.

The user flow is exactly the existing flow — parse the path, boot the TUI against it — with the single addition that `-w` is accepted as an alternate spelling of `--workflow-dir`. Both spellings MUST resolve to the same `workflow_dir: PathBuf` field on `Cli`, both MUST honor the same `default_value = "."`, and the auto-generated `-h` / `--help` output MUST surface `-w` alongside `--workflow-dir` so users discover it without reading source.

## Acceptance criteria

**AC-1 — `spacetop -w <path>` parses identically to `spacetop --workflow-dir <path>`.**
Verified by a `cargo test` unit test in `src/cli.rs` that calls `Cli::parse_from(["spacetop", "-w", "docs/spacetop-dev"])` and asserts `cli.workflow_dir == PathBuf::from("docs/spacetop-dev")`.

**AC-2 — The existing long-form `--workflow-dir <path>` continues to parse unchanged.**
Verified by the existing `parses_workflow_dir` test continuing to pass without modification.

**AC-3 — Omitting the flag still defaults `workflow_dir` to `"."`.**
Verified by the existing `defaults_workflow_dir_to_current_directory` test continuing to pass.

**AC-4 — `spacetop -h` (and `--help`) lists the short form `-w` alongside `--workflow-dir` in the flag description line.**
Verified by a unit test that renders the help string via `Cli::command().render_help()` (or `.render_long_help()`) and asserts it contains both `-w` and `--workflow-dir` on the workflow-dir entry. Manual spot-check of `cargo run -- -h` is acceptable supporting evidence.

**AC-5 — `clap`'s built-in debug assertions still pass.**
Verified by the existing `clap_definition_is_valid` test continuing to pass (catches short-flag collisions or malformed `#[arg(...)]` attributes).

## Parser / TUI constraints

- Implementation is confined to `src/cli.rs` — add `short = 'w'` to the existing `#[arg(long, ...)]` attribute on `workflow_dir`. No new module, no new dependency.
- No TUI change. This is CLI parsing only; the downstream consumer (`workflow_dir: PathBuf`) is untouched.
- No workflow parser change. The spacedock snapshot parser does not see the flag — it only sees the resolved `PathBuf`.
- No short-flag collisions today (`-w` is the only short flag on the struct), but AC-5 keeps us honest if new flags are added later.

## Implementation plan

### Step 1 — Add `short = 'w'` to the clap attribute in `src/cli.rs`

Change the single `#[arg(...)]` attribute on `Cli::workflow_dir` (currently line 13) from:

```rust
#[arg(long, value_name = "PATH", default_value = ".")]
```

to:

```rust
#[arg(short = 'w', long, value_name = "PATH", default_value = ".")]
```

No other edits in `src/cli.rs`. No changes to the field type, the field name, the `default_value`, or the `value_name`. No touch to `src/main.rs`, `src/app.rs`, `src/ui.rs`, or the parser module.

### Step 2 — Add two new tests in the existing `#[cfg(test)] mod tests` block

Add these tests alongside the existing `clap_definition_is_valid`, `parses_workflow_dir`, and `defaults_workflow_dir_to_current_directory`:

1. `parses_workflow_dir_short_flag` — asserts `Cli::parse_from(["spacetop", "-w", "docs/spacetop-dev"]).workflow_dir == PathBuf::from("docs/spacetop-dev")`. Covers AC-1.
2. `help_output_surfaces_both_spellings` — renders help via `Cli::command().render_help().to_string()` and asserts the string contains both `-w` and `--workflow-dir`. Covers AC-4.

The existing `parses_workflow_dir` test (AC-2), `defaults_workflow_dir_to_current_directory` test (AC-3), and `clap_definition_is_valid` test (AC-5) are left unmodified and must continue to pass.

### Step 3 — Verification commands

Run from repo root, in order:

1. `cargo fmt --check` — formatting is clean.
2. `cargo test` — all five tests in `src/cli::tests` pass (3 existing + 2 new).
3. `cargo run -- -w docs/spacetop-dev --help | cat` — smoke-check that `-w` is accepted by the parser at runtime and that the rendered help output surfaces `-w, --workflow-dir <PATH>` on the flag line.

Expected help snippet (clap default rendering):

```
-w, --workflow-dir <PATH>   Path to a Spacedock workflow directory. [default: .]
```

## Test strategy

| Test | Covers | New? | File |
| --- | --- | --- | --- |
| `clap_definition_is_valid` | AC-5 (no short-flag collisions, attrs well-formed) | existing | `src/cli.rs` |
| `parses_workflow_dir` | AC-2 (`--workflow-dir <path>` still parses) | existing | `src/cli.rs` |
| `defaults_workflow_dir_to_current_directory` | AC-3 (default remains `.`) | existing | `src/cli.rs` |
| `parses_workflow_dir_short_flag` | AC-1 (`-w <path>` parses identically) | NEW | `src/cli.rs` |
| `help_output_surfaces_both_spellings` | AC-4 (help lists both spellings) | NEW | `src/cli.rs` |

Evidence that suffices for stage completion: `cargo test` prints `5 passed; 0 failed` for the cli module, and the `cargo run -- -h` smoke output contains `-w, --workflow-dir`.

## File / module ownership

- Implement stage edits only `src/cli.rs`.
- No edits to: `src/main.rs`, `src/app.rs`, `src/ui.rs`, `src/workflow/*`, `Cargo.toml`, or any docs under `docs/`.
- No CLI surface renaming: the field stays `workflow_dir: PathBuf`, the long flag stays `--workflow-dir`, the default stays `"."`, and the help text on the field stays unchanged.
- No new crate dependencies; `clap`'s derive already supports `short = 'w'`.

## Stage Report: design

- DONE: Problem statement and user flow confirm the short-flag behavior exactly mirrors `--workflow-dir`, including in `-h`/`--help` output.
  Two-paragraph "Problem statement and user flow" section states both spellings resolve to the same `workflow_dir` field, share the default, and that `-w` must appear in help output.
- DONE: Acceptance criteria replace the placeholder section with concrete, verifiable AC-N bullets covering both spellings and help-text surfacing.
  Placeholder removed; AC-1 through AC-5 added, covering `-w` parse, `--workflow-dir` parse, default, `-h` surfacing of both spellings, and clap debug assertions.
- DONE: Parser/TUI constraints are named (expected: `src/cli.rs` only, no TUI or parser changes).
  "Parser / TUI constraints" section names `src/cli.rs` as the sole edit site, confirms no TUI or workflow-parser change, and flags short-flag collision risk as covered by AC-5.

### Summary

Firmed up the design for the `-w` short alias: it is a pure `clap` derive annotation in `src/cli.rs` (add `short = 'w'` to the existing `#[arg(long, ...)]`), with no TUI, parser, or behavior change. Acceptance criteria explicitly require parity tests for both spellings, default preservation, and that `-h` surfaces `-w` alongside `--workflow-dir` so the alias is discoverable. Verified the current `src/cli.rs` shape (single field, clap derive, no existing short flags) to confirm there is no collision risk today.

## Stage Report: plan

- DONE: Step-by-step plan names the exact `src/cli.rs` change and the verification commands (`cargo fmt --check`, `cargo test`, `cargo run -- -w docs/spacetop-dev --help | cat`).
  "Implementation plan" section shows the before/after clap attribute edit on `Cli::workflow_dir` and lists the three verification commands in order in Step 3.
- DONE: Test strategy names the specific tests to add — covering `-w <path>` parsing, `--workflow-dir <path>` still parses, help output surfaces both spellings.
  "Test strategy" table names `parses_workflow_dir_short_flag` (new, AC-1) and `help_output_surfaces_both_spellings` (new, AC-4), and confirms the three existing tests cover AC-2, AC-3, AC-5 unchanged.
- DONE: File/module ownership: implement stage touches `src/cli.rs` only; no parser, app, UI, or CLI surface renaming.
  "File / module ownership" section lists `src/cli.rs` as the only edit site, enumerates excluded files (`main.rs`, `app.rs`, `ui.rs`, `workflow/*`, `Cargo.toml`, docs), and confirms no field/flag/default rename.

### Summary

Plan is minimal and proportional: one clap attribute edit (`short = 'w'` added to the existing `#[arg(long, ...)]`) plus two new unit tests in the same `src/cli.rs` `#[cfg(test)]` block — one for `-w <path>` parsing and one asserting help output surfaces both spellings. Verification is the standard triad (`cargo fmt --check`, `cargo test`, `cargo run -- -w docs/spacetop-dev --help | cat`). No other files are touched; no CLI surface renaming occurs.
