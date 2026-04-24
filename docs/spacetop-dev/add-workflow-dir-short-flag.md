---
id: 004
title: Add short `-w` alias for `--workflow-dir`
status: plan
source: captain feedback after build-initial-tui-overview
started: 2026-04-24T16:04:53Z
completed:
verdict:
score:
worktree:
issue:
pr:
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

## Stage Report: design

- DONE: Problem statement and user flow confirm the short-flag behavior exactly mirrors `--workflow-dir`, including in `-h`/`--help` output.
  Two-paragraph "Problem statement and user flow" section states both spellings resolve to the same `workflow_dir` field, share the default, and that `-w` must appear in help output.
- DONE: Acceptance criteria replace the placeholder section with concrete, verifiable AC-N bullets covering both spellings and help-text surfacing.
  Placeholder removed; AC-1 through AC-5 added, covering `-w` parse, `--workflow-dir` parse, default, `-h` surfacing of both spellings, and clap debug assertions.
- DONE: Parser/TUI constraints are named (expected: `src/cli.rs` only, no TUI or parser changes).
  "Parser / TUI constraints" section names `src/cli.rs` as the sole edit site, confirms no TUI or workflow-parser change, and flags short-flag collision risk as covered by AC-5.

### Summary

Firmed up the design for the `-w` short alias: it is a pure `clap` derive annotation in `src/cli.rs` (add `short = 'w'` to the existing `#[arg(long, ...)]`), with no TUI, parser, or behavior change. Acceptance criteria explicitly require parity tests for both spellings, default preservation, and that `-h` surfaces `-w` alongside `--workflow-dir` so the alias is discoverable. Verified the current `src/cli.rs` shape (single field, clap derive, no existing short flags) to confirm there is no collision risk today.
