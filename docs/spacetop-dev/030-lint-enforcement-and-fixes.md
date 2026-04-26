---
id: "030"
title: "Add lint enforcement to CLAUDE.md/AGENTS.md and fix all clippy errors"
status: review
source: feature request
started: 2026-04-26T07:07:02Z
completed:
verdict:
score: 0.9
worktree: .worktrees/spacedock-ensign-030-lint-enforcement-and-fixes
issue:
pr:
mod-block: merge:pr-merge
---

`make lint` now runs `cargo clippy --all-targets --all-features -- -D warnings` (enforced via the Makefile). Agents are not currently instructed to run it before completing a task, causing lint regressions to accumulate undetected (16 errors exist today). This task adds the lint enforcement rule to both AGENTS.md and CLAUDE.md and fixes every current clippy error.

## Current lint errors (16)

**src/domain/mod.rs — 14 × `clippy::excessive_precision`**
The oklch-to-sRGB conversion constants have more decimal digits than an f64 can represent. Clippy truncates them at compile time but flags it as excessive precision.

**src/ui/mod.rs — 1 × `clippy::collapsible_match`**
A nested `if let` inside a `match` arm can be collapsed into the outer match.

**src/ui/mod.rs (tests) — 1 × `clippy::useless_vec`**
A `vec![...]` literal is used where a slice `[...]` suffices.

## Changes required

### 1. AGENTS.md — add lint enforcement section

Append to the existing AGENTS.md:

```markdown
## Lint Gate

Before marking any task complete, run:

    make lint

This runs `cargo clippy --all-targets --all-features -- -D warnings`. All warnings are errors. Fix every clippy diagnostic before committing the stage report.
```

### 2. CLAUDE.md — create with lint enforcement

Create CLAUDE.md at the repo root with project context and the same lint gate rule:

```markdown
# Spacetop

Rust TUI for browsing Spacedock workflow state.

## Lint Gate

Before marking any task complete, run:

    make lint

All clippy warnings are treated as errors (`-D warnings`). Fix every diagnostic before committing.

## Commands

| Command | Purpose |
|---------|---------|
| `make build` | Release build (also runs lint) |
| `make lint` | Run clippy only |
| `make install` | Build and install to `~/.cargo/bin` |
| `cargo test` | Run all tests |
```

### 3. Fix all 16 clippy errors

- Truncate the 14 excessive-precision float literals in `src/domain/mod.rs`
- Collapse the nested `if let` in `src/ui/mod.rs`
- Replace the `vec![...]` with a slice in the test

## Acceptance criteria

**AC-1 -- make lint exits 0.**
`make lint` completes with no errors or warnings.
Verified by: `make lint 2>&1; echo "exit: $?"`

**AC-2 -- AGENTS.md contains the lint gate rule.**
`grep -A3 "Lint Gate" AGENTS.md` shows the `make lint` instruction.

**AC-3 -- CLAUDE.md exists and contains the lint gate rule.**
`grep -A3 "Lint Gate" CLAUDE.md` shows the `make lint` instruction.

**AC-4 -- All tests still pass.**
`cargo test` 174/174 (or current count) green.

## Stage Report: implement

- DONE: AGENTS.md updated with Lint Gate section (make lint instruction).
  Appended to existing AGENTS.md; `grep -A3 "Lint Gate" AGENTS.md` confirms the `make lint` instruction.
- DONE: CLAUDE.md created at repo root with project context and Lint Gate section.
  Created at repo root; `grep -A3 "Lint Gate" CLAUDE.md` confirms the `make lint` instruction.
- DONE: All 16 clippy errors fixed: 14 excessive_precision in src/domain/mod.rs, 1 collapsible_match in src/ui/mod.rs, 1 useless_vec in tests.
  Also fixed 1 additional `assertions_on_constants` in src/main.rs that was exposed once the lib errors cleared. `make lint` exits 0.
- DONE: make lint exits 0; cargo test passes.
  `make lint` exits 0; `cargo test` 174 passed.

### Summary

Added lint enforcement instructions to AGENTS.md (appended Lint Gate section) and created CLAUDE.md at repo root with project context and lint gate rule. Fixed all 16 clippy errors: truncated 14 excessive-precision f32 literals in `src/domain/mod.rs`, collapsed a nested `if` into a match guard in `src/ui/mod.rs`, and replaced a `vec![...]` with an array literal in the test. A 17th pre-existing lint error (`assertions_on_constants` in `src/main.rs`) was also fixed since it was blocking `make lint` from exiting 0.

## Stage Report: review

- DONE: AGENTS.md contains a Lint Gate section with make lint instruction.
  `grep -A3 "Lint Gate" AGENTS.md` confirms `make lint` present; commit e0da5f4.
- DONE: CLAUDE.md exists at repo root and contains a Lint Gate section with make lint instruction.
  `grep -A3 "Lint Gate" CLAUDE.md` confirms `make lint` present; commit e0da5f4.
- DONE: make lint exits 0 — all clippy errors resolved (run it to verify).
  `make lint` exits 0 (no diagnostics, finished in 0.43s).
- DONE: cargo test passes.
  174 + 4 + 8 = 186 tests passed, 0 failed.

### Summary

Reviewed the implement commit (e0da5f4) on branch `spacedock-ensign/030-lint-enforcement-and-fixes`. All four acceptance criteria pass: AGENTS.md and CLAUDE.md both contain the Lint Gate section with `make lint` instruction, `make lint` exits 0 with no diagnostics, and all 186 tests pass across unit, binary, and integration test targets.
